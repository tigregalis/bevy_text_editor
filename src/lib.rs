pub mod prelude {
    pub use crate::plugin::*;
}

mod plugin {

    use std::cmp;
    use std::collections::{HashMap, VecDeque};
    use std::time::{Duration, Instant};

    use bevy::ecs::system::SystemParam;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::input::ButtonState;
    use bevy::prelude::*;
    use bevy::render::{Extract, ExtractSchedule, RenderApp};
    use bevy::text::cosmic_text::{
        Action, Buffer, Cursor, Edit, Editor, LayoutRun, Motion, Selection,
    };
    use bevy::text::{BreakLineOn, CosmicBuffer, TextLayoutInfo};
    use bevy::ui::widget::TextFlags;
    use bevy::ui::{
        ContentSize, ExtractedUiNode, ExtractedUiNodes, FocusPolicy, NodeType, RenderUiSystem,
    };
    use bevy::window::PrimaryWindow;
    use unicode_segmentation::UnicodeSegmentation as _;

    pub struct TextEditorPlugin;

    impl Plugin for TextEditorPlugin {
        fn build(&self, app: &mut App) {
            app.add_systems(
                PreUpdate,
                (hit.pipe(handle_click), listen_keyboard_input_events),
            );
            let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
                return;
            };
            render_app.add_systems(
                ExtractSchedule,
                (
                    extract_selection.before(RenderUiSystem::ExtractText),
                    extract_cursor.after(RenderUiSystem::ExtractText),
                ),
            );
        }
    }

    /// A UI node that is text and editable
    ///
    /// The positioning of this node is controlled by the UI layout system. If you need manual control,
    /// use [`TextEditor2dBundle`].
    #[derive(Bundle, Debug, Default)]
    pub struct TextEditorBundle {
        /// Describes the logical size of the node
        pub node: Node,
        /// Styles which control the layout (size and position) of the node and its children
        /// In some cases these styles also affect how the node drawn/painted.
        pub style: Style,
        /// Contains the text of the node
        pub text: Text,
        /// Cached cosmic buffer for layout
        pub buffer: CosmicBuffer,
        /// Text layout information
        pub text_layout_info: TextLayoutInfo,
        /// Text system flags
        pub text_flags: TextFlags,
        /// The calculated size based on the given image
        pub calculated_size: ContentSize,
        /// Whether this node should block interaction with lower nodes
        pub focus_policy: FocusPolicy,
        /// The transform of the node
        ///
        /// This component is automatically managed by the UI layout system.
        /// To alter the position of the `TextBundle`, use the properties of the [`Style`] component.
        pub transform: Transform,
        /// The global transform of the node
        ///
        /// This component is automatically updated by the [`TransformPropagate`](`bevy_transform::TransformSystem::TransformPropagate`) systems.
        pub global_transform: GlobalTransform,
        /// Describes the visibility properties of the node
        pub visibility: Visibility,
        /// Inherited visibility of an entity.
        pub inherited_visibility: InheritedVisibility,
        /// Algorithmically-computed indication of whether an entity is visible and should be extracted for rendering
        pub view_visibility: ViewVisibility,
        /// Indicates the depth at which the node should appear in the UI
        pub z_index: ZIndex,
        /// The background color that will fill the containing node
        pub background_color: BackgroundColor,
        pub editor_state: EditorState,
        pub cursor_config: CursorConfig,
        pub selection_config: SelectionConfig,
    }

    impl TextEditorBundle {
        /// Create a [`TextBundle`] from a single section.
        ///
        /// See [`Text::from_section`] for usage.
        pub fn from_section(value: impl Into<String>, style: TextStyle) -> Self {
            Self {
                text: Text::from_section(value, style),
                ..Default::default()
            }
        }

        /// Create a [`TextBundle`] from a list of sections.
        ///
        /// See [`Text::from_sections`] for usage.
        pub fn from_sections(sections: impl IntoIterator<Item = TextSection>) -> Self {
            Self {
                text: Text::from_sections(sections),
                ..Default::default()
            }
        }

        /// Returns this [`TextBundle`] with a new [`JustifyText`] on [`Text`].
        pub const fn with_text_justify(mut self, justify: JustifyText) -> Self {
            self.text.justify = justify;
            self
        }

        /// Returns this [`TextBundle`] with a new [`Style`].
        pub fn with_style(mut self, style: Style) -> Self {
            self.style = style;
            self
        }

        /// Returns this [`TextBundle`] with a new [`BackgroundColor`].
        pub const fn with_background_color(mut self, color: Color) -> Self {
            self.background_color = BackgroundColor(color);
            self
        }

        /// Returns this [`TextBundle`] with soft wrapping disabled.
        /// Hard wrapping, where text contains an explicit linebreak such as the escape sequence `\n`, will still occur.
        pub const fn with_no_wrap(mut self) -> Self {
            self.text.linebreak_behavior = BreakLineOn::NoWrap;
            self
        }
    }

    impl<I> From<I> for TextEditorBundle
    where
        I: Into<TextSection>,
    {
        fn from(value: I) -> Self {
            Self::from_sections(vec![value.into()])
        }
    }

    #[derive(Debug)]
    pub struct ClickHistoryEntry {
        pub position: Vec2,
        pub time: Instant,
    }

    #[derive(Debug)]
    pub struct ClickHistory {
        pub history: VecDeque<ClickHistoryEntry>,
    }

    impl Default for ClickHistory {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ClickHistory {
        const MAX_ENTRIES: usize = 4;
        const MAX_DISTANCE: f32 = 2.0;
        const MAX_INTERVAL: Duration = Duration::from_millis(500);

        pub fn new() -> Self {
            Self {
                history: VecDeque::with_capacity(Self::MAX_ENTRIES),
            }
        }

        pub fn add_entry(&mut self, position: Vec2) {
            // drop down to the most recent entries, with room for one more
            while self.history.len() >= Self::MAX_ENTRIES {
                self.history.pop_back();
            }
            // add the new entry
            self.history.push_front(ClickHistoryEntry {
                position,
                time: Instant::now(),
            });
        }

        pub fn clicked(&self, times: usize) -> bool {
            let len = self.history.len();
            if len < times {
                return false;
            }
            let mut iter = self.history.iter().take(times).peekable();
            while let Some(a) = iter.next() {
                if let Some(b) = iter.peek() {
                    debug_assert!(a.time > b.time);
                    if a.position.distance(b.position) > Self::MAX_DISTANCE {
                        return false;
                    }
                    if a.time - b.time > Self::MAX_INTERVAL {
                        return false;
                    }
                }
            }
            true
        }
    }

    /// Piped from [`hit`]
    ///
    /// TODO: This should respect UI stack indexes / Z ordering
    #[allow(clippy::type_complexity)]
    pub fn handle_click(
        In(hit): In<Option<HitOutput>>,
        mut click_history: Local<ClickHistory>,
        mouse_button: Res<ButtonInput<MouseButton>>,
        mut buffer: Query<(&mut CosmicBuffer, &mut EditorState), With<Text>>,
        mut text_pipeline: ResMut<bevy::text::TextPipeline>,
    ) {
        if !mouse_button.just_pressed(MouseButton::Left) {
            return;
        }
        let Some(HitOutput {
            entity: parent,
            span_index: _,
            position,
        }) = hit
        else {
            return;
        };
        click_history.add_entry(position);

        let Ok((mut buf, mut editor_state)) = buffer.get_mut(parent) else {
            return;
        };
        editor_state.resume(&mut buf).with_editor_mut(|editor| {
            let font_system = text_pipeline.font_system_mut();
            if click_history.clicked(3) {
                info!("triple-click: {click_history:?}");
                editor.action(
                    font_system,
                    Action::TripleClick {
                        x: position.x as i32,
                        y: position.y as i32,
                    },
                );
            } else if click_history.clicked(2) {
                info!("double-click: {click_history:?}");
                editor.action(
                    font_system,
                    Action::DoubleClick {
                        x: position.x as i32,
                        y: position.y as i32,
                    },
                );
            } else if click_history.clicked(1) {
                info!("single-click: {click_history:?}");
                editor.action(
                    font_system,
                    Action::Click {
                        x: position.x as i32,
                        y: position.y as i32,
                    },
                );
            } else {
                unreachable!("clicked but zero clicks?");
            }
        });
    }

    pub fn listen_keyboard_input_events(
        mut events: EventReader<KeyboardInput>,
        mut buffer: Query<(&mut CosmicBuffer, &mut Text, &mut EditorState)>,
        mut text_pipeline: ResMut<bevy::text::TextPipeline>,
        mut scratch_spans_for_deletion: Local<Vec<usize>>,
        mut scratch_spans_for_update: Local<HashMap<usize, String>>,
    ) {
        for event in events.read() {
            // Only trigger changes when the key is first pressed.
            if event.state == ButtonState::Released {
                continue;
            }

            for (mut buf, mut text, mut editor_state) in &mut buffer {
                editor_state.resume(&mut buf).with_editor_mut(|editor| {
                    let font_system = text_pipeline.font_system_mut();
                    // info!("Before: {:?}", editor.cursor());
                    match &event.logical_key {
                        Key::Character(character) => {
                            for c in character.chars() {
                                editor.action(font_system, Action::Insert(c));
                            }
                        }
                        Key::Enter => editor.action(font_system, Action::Enter),
                        Key::Space => editor.action(font_system, Action::Insert(' ')),
                        Key::Backspace => editor.action(font_system, Action::Backspace),
                        Key::Delete => editor.action(font_system, Action::Delete),
                        Key::Control => {
                            info!("TODO: Control");
                        }
                        Key::Shift => {
                            info!("TODO: Shift");
                        }
                        Key::Tab => {
                            info!("TODO: Tab");
                        }
                        Key::ArrowDown => editor.action(font_system, Action::Motion(Motion::Down)),
                        Key::ArrowLeft => editor.action(font_system, Action::Motion(Motion::Left)),
                        Key::ArrowRight => {
                            editor.action(font_system, Action::Motion(Motion::Right))
                        }
                        Key::ArrowUp => editor.action(font_system, Action::Motion(Motion::Up)),
                        Key::End => editor.action(font_system, Action::Motion(Motion::End)),
                        Key::Home => editor.action(font_system, Action::Motion(Motion::Home)),
                        Key::PageDown => {
                            editor.action(font_system, Action::Motion(Motion::PageDown))
                        }
                        Key::PageUp => editor.action(font_system, Action::Motion(Motion::PageUp)),
                        _ => {}
                    }
                });

                // rebuild the text from scratch
                for line in &buf.lines {
                    let line_text = line.text();
                    let len = line_text.len();
                    let ending = line.ending().as_str();
                    let spans = line.attrs_list().spans();
                    // NOTE: cosmic-text allows for "unstyled" (default-styled) spans/ranges
                    //       this means not all `spans` actually have styles
                    //       so imagine a line with 21 characters (full range 0..21)
                    //       the `spans` iterator can yield for example 2..7, 9..12, 12..13, 13..16, 17..19
                    //       so 0..2, 7..9, 16..17, 19..21 are unstyled, and we have to specially handle these
                    //       in this case, we will style
                    //       0..2 like 2..7 (unstyled span will be styled like next styled span)
                    //       7..9 like 9..12 (unstyled span will be styled like next styled span)
                    //       16..17 like 17..19 (unstyled span will be styled like next styled span)
                    //       19..21 like 17..19 (final part of line, unstyled span will be styled like previous styled span)
                    let mut current_pos = 0;
                    let mut bevy_span_index = 0;
                    for (range, attrs) in spans.into_iter() {
                        bevy_span_index = attrs.metadata;
                        let s = scratch_spans_for_update.entry(bevy_span_index).or_default();
                        // "unstyled" spans will take the following range's attrs
                        if current_pos < range.start {
                            s.push_str(&line_text[current_pos..range.start]);
                        }
                        // push the styled span
                        s.push_str(&line_text[range.clone()]);
                        current_pos = range.end;
                        // push the line ending if we've reached the end of the line
                        if current_pos == len {
                            s.push_str(ending);
                        }
                    }
                    // final part of the line
                    if current_pos < len {
                        let s = scratch_spans_for_update.entry(bevy_span_index).or_default();
                        // push the styled span
                        s.push_str(&line_text[current_pos..len]);
                        // push the line ending since we've reached the end of the line
                        s.push_str(ending);
                    }
                }

                // apply the changes (well, everything) to the text component
                for i in 0..text.sections.len() {
                    match scratch_spans_for_update.remove(&i) {
                        // TODO: should be forwarded to the TextSpan component for child spans instead
                        // TODO: could be more efficient (don't update the whole string if no changes were made)
                        Some(s) => text.sections[i].value = s,
                        None => scratch_spans_for_deletion.push(i),
                    }
                }
                scratch_spans_for_deletion.reverse();
                for i in scratch_spans_for_deletion.drain(..) {
                    if text.sections.len() > 1 {
                        text.sections.remove(i);
                    } else {
                        text.sections[0].value = String::new();
                    }
                }
            }
        }
    }

    /// Adapted from `bevy_ui::extract_uinode_text` and `bevy_ui::extract_uinode_background_colors`
    #[allow(clippy::type_complexity)]
    pub fn extract_cursor(
        mut commands: Commands,
        mut extracted_uinodes: ResMut<ExtractedUiNodes>,
        camera_query: Extract<Query<(Entity, &Camera)>>,
        default_ui_camera: Extract<DefaultUiCamera>,
        ui_scale: Extract<Res<UiScale>>,
        // TODO: the cursor should be its own entity!
        uinode_query: Extract<
            Query<
                (
                    &Node,
                    &GlobalTransform,
                    &ViewVisibility,
                    Option<&CalculatedClip>,
                    Option<&TargetCamera>,
                    Option<&CursorConfig>,
                    &CosmicBuffer,
                    &EditorState,
                ),
                With<Text>,
            >,
        >,
    ) {
        for (
            uinode,
            global_transform,
            view_visibility,
            clip,
            camera,
            cursor_config,
            buffer,
            editor_state,
        ) in &uinode_query
        {
            let Some(cursor) = editor_state.cursor else {
                continue;
            };

            let Some(camera_entity) = camera.map(TargetCamera::entity).or(default_ui_camera.get())
            else {
                continue;
            };

            // Skip if not visible or if size is set to zero (e.g. when a parent is set to `Display::None`)
            if !view_visibility.get() || uinode.size().x == 0. || uinode.size().y == 0. {
                continue;
            }

            let scale_factor = camera_query
                .get(camera_entity)
                .ok()
                .and_then(|(_, c)| c.target_scaling_factor())
                .unwrap_or(1.0)
                * ui_scale.0;
            let inverse_scale_factor = scale_factor.recip();

            // Align the text to the nearest physical pixel:
            // * Translate by minus the text node's half-size
            //      (The transform translates to the center of the node but the text coordinates are relative to the node's top left corner)
            // * Multiply the logical coordinates by the scale factor to get its position in physical coordinates
            // * Round the physical position to the nearest physical pixel
            // * Multiply by the rounded physical position by the inverse scale factor to return to logical coordinates

            let logical_top_left = -0.5 * uinode.size();

            let mut transform = global_transform.affine()
                * bevy::math::Affine3A::from_translation(logical_top_left.extend(0.));

            transform.translation *= scale_factor;
            transform.translation = transform.translation.round();
            transform.translation *= inverse_scale_factor;

            let cursor_config = match cursor_config {
                Some(c) => *c,
                None => Default::default(),
            };
            let color = cursor_config.color.into();
            let width = cursor_config.width;

            // TODO: we can locate the exact layout_run by the cursor position
            for run in buffer.layout_runs() {
                // TODO: this should happen in the main world so that we do as little work as possible here
                if let Some((x, y)) = cursor_position(&cursor, &run) {
                    let position = Vec2::new(x as f32, y as f32 + run.line_height / 2.0);
                    extracted_uinodes.uinodes.insert(
                        commands.spawn_empty().id(),
                        ExtractedUiNode {
                            stack_index: uinode.stack_index(),
                            transform: transform
                                * Mat4::from_translation(
                                    position.extend(0.) * inverse_scale_factor,
                                ),
                            color,
                            rect: Rect {
                                min: Vec2::ZERO,
                                // TODO: size?
                                max: Vec2::new(width, run.line_height),
                            },
                            image: AssetId::default(),
                            atlas_size: None,
                            clip: clip.map(|clip| clip.clip),
                            flip_x: false,
                            flip_y: false,
                            camera_entity,
                            border: [0.; 4],
                            border_radius: [0.; 4],
                            node_type: NodeType::Rect,
                        },
                    );
                }
            }
        }
    }

    /// Adapted from `bevy_ui::extract_uinode_text` and `bevy_ui::extract_uinode_background_colors`
    #[allow(clippy::type_complexity)]
    pub fn extract_selection(
        mut commands: Commands,
        mut extracted_uinodes: ResMut<ExtractedUiNodes>,
        camera_query: Extract<Query<(Entity, &Camera)>>,
        default_ui_camera: Extract<DefaultUiCamera>,
        ui_scale: Extract<Res<UiScale>>,
        // TODO: the selection should be its own entity!?
        uinode_query: Extract<
            Query<
                (
                    &Node,
                    &GlobalTransform,
                    &ViewVisibility,
                    Option<&CalculatedClip>,
                    Option<&TargetCamera>,
                    Option<&SelectionConfig>,
                    &CosmicBuffer,
                    &EditorState,
                ),
                With<Text>,
            >,
        >,
    ) {
        for (
            uinode,
            global_transform,
            view_visibility,
            clip,
            camera,
            selection_config,
            buffer,
            editor_state,
        ) in &uinode_query
        {
            if editor_state.selection == Selection::None {
                continue;
            };
            let Some(camera_entity) = camera.map(TargetCamera::entity).or(default_ui_camera.get())
            else {
                continue;
            };

            // Skip if not visible or if size is set to zero (e.g. when a parent is set to `Display::None`)
            if !view_visibility.get() || uinode.size().x == 0. || uinode.size().y == 0. {
                continue;
            }

            let scale_factor = camera_query
                .get(camera_entity)
                .ok()
                .and_then(|(_, c)| c.target_scaling_factor())
                .unwrap_or(1.0)
                * ui_scale.0;
            let inverse_scale_factor = scale_factor.recip();

            // Align the text to the nearest physical pixel:
            // * Translate by minus the text node's half-size
            //      (The transform translates to the center of the node but the text coordinates are relative to the node's top left corner)
            // * Multiply the logical coordinates by the scale factor to get its position in physical coordinates
            // * Round the physical position to the nearest physical pixel
            // * Multiply by the rounded physical position by the inverse scale factor to return to logical coordinates

            let logical_top_left = -0.5 * uinode.size();

            let mut transform = global_transform.affine()
                * bevy::math::Affine3A::from_translation(logical_top_left.extend(0.));

            transform.translation *= scale_factor;
            transform.translation = transform.translation.round();
            transform.translation *= inverse_scale_factor;

            let selection_config = match selection_config {
                Some(c) => *c,
                None => Default::default(),
            };
            let color = selection_config.color.into();

            for run in buffer.layout_runs() {
                // TODO: this should happen in the main world so that we do as little work as possible here
                if let Some((x, y, width)) =
                    highlight_selection(editor_state.selection_bounds, buffer.size().0, &run)
                {
                    let position = Vec2::new(
                        x as f32 + width as f32 / 2.0,
                        y as f32 + run.line_height / 2.0,
                    );
                    extracted_uinodes.uinodes.insert(
                        commands.spawn_empty().id(),
                        ExtractedUiNode {
                            stack_index: uinode.stack_index(),
                            transform: transform
                                * Mat4::from_translation(
                                    position.extend(0.) * inverse_scale_factor,
                                ),
                            color,
                            rect: Rect {
                                min: Vec2::ZERO,
                                // TODO: size?
                                max: Vec2::new(width as f32, run.line_height),
                            },
                            image: AssetId::default(),
                            atlas_size: None,
                            clip: clip.map(|clip| clip.clip),
                            flip_x: false,
                            flip_y: false,
                            camera_entity,
                            border: [0.; 4],
                            border_radius: [0.; 4],
                            node_type: NodeType::Rect,
                        },
                    );
                }
            }
        }
    }

    // from cosmic-text/src/edit/editor.rs:66
    pub fn cursor_position(cursor: &Cursor, run: &LayoutRun) -> Option<(i32, i32)> {
        let (cursor_glyph, cursor_glyph_offset) = cursor_glyph_opt(cursor, run)?;
        let x = match run.glyphs.get(cursor_glyph) {
            Some(glyph) => {
                // Start of detected glyph
                if glyph.level.is_rtl() {
                    (glyph.x + glyph.w - cursor_glyph_offset) as i32
                } else {
                    (glyph.x + cursor_glyph_offset) as i32
                }
            }
            None => match run.glyphs.last() {
                Some(glyph) => {
                    // End of last glyph
                    if glyph.level.is_rtl() {
                        glyph.x as i32
                    } else {
                        (glyph.x + glyph.w) as i32
                    }
                }
                None => {
                    // Start of empty line
                    0
                }
            },
        };

        Some((x, run.line_top as i32))
    }

    // adapted from cosmic-text/src/edit/editor.rs:?
    pub fn highlight_selection(
        selection_bounds: Option<(Cursor, Cursor)>,
        buffer_width: Option<f32>,
        run: &LayoutRun,
    ) -> Option<(i32, i32, u32)> {
        let line_i = run.line_i;
        let line_top = run.line_top;

        // Highlight selection
        if let Some((start, end)) = selection_bounds {
            if line_i >= start.line && line_i <= end.line {
                let mut range_opt = None;
                for glyph in run.glyphs.iter() {
                    // Guess x offset based on characters
                    let cluster = &run.text[glyph.start..glyph.end];
                    let total = cluster.grapheme_indices(true).count();
                    let mut c_x = glyph.x;
                    let c_w = glyph.w / total as f32;
                    for (i, c) in cluster.grapheme_indices(true) {
                        let c_start = glyph.start + i;
                        let c_end = glyph.start + i + c.len();
                        if (start.line != line_i || c_end > start.index)
                            && (end.line != line_i || c_start < end.index)
                        {
                            range_opt = match range_opt.take() {
                                Some((min, max)) => Some((
                                    cmp::min(min, c_x as i32),
                                    cmp::max(max, (c_x + c_w) as i32),
                                )),
                                None => Some((c_x as i32, (c_x + c_w) as i32)),
                            };
                        } else if let Some((min, max)) = range_opt.take() {
                            return Some((min, line_top as i32, cmp::max(0, max - min) as u32));
                        }
                        c_x += c_w;
                    }
                }

                if run.glyphs.is_empty() && end.line > line_i {
                    // Highlight all of internal empty lines
                    range_opt = Some((0, buffer_width.unwrap_or(0.0) as i32));
                }

                if let Some((mut min, mut max)) = range_opt.take() {
                    if end.line > line_i {
                        // Draw to end of line
                        if run.rtl {
                            min = 0;
                        } else {
                            max = buffer_width.unwrap_or(0.0) as i32;
                        }
                    }
                    return Some((min, line_top as i32, cmp::max(0, max - min) as u32));
                }
            }
        }
        None
    }

    // from cosmic-text/src/edit/editor.rs:30
    pub fn cursor_glyph_opt(cursor: &Cursor, run: &LayoutRun) -> Option<(usize, f32)> {
        if cursor.line == run.line_i {
            for (glyph_i, glyph) in run.glyphs.iter().enumerate() {
                if cursor.index == glyph.start {
                    return Some((glyph_i, 0.0));
                } else if cursor.index > glyph.start && cursor.index < glyph.end {
                    // Guess x offset based on characters
                    let mut before = 0;
                    let mut total = 0;

                    let cluster = &run.text[glyph.start..glyph.end];
                    for (i, _) in cluster.grapheme_indices(true) {
                        if glyph.start + i < cursor.index {
                            before += 1;
                        }
                        total += 1;
                    }

                    let offset = glyph.w * (before as f32) / (total as f32);
                    return Some((glyph_i, offset));
                }
            }
            match run.glyphs.last() {
                Some(glyph) => {
                    if cursor.index == glyph.end {
                        return Some((run.glyphs.len(), 0.0));
                    }
                }
                None => {
                    return Some((0, 0.0));
                }
            }
        }
        None
    }

    #[derive(Component, Clone, Copy, Debug)]
    pub struct EditorState {
        pub cursor: Option<Cursor>,
        pub selection: Selection,
        pub selection_bounds: Option<(Cursor, Cursor)>,
    }

    impl Default for EditorState {
        fn default() -> Self {
            Self {
                cursor: None,
                selection: Selection::None,
                selection_bounds: None,
            }
        }
    }

    impl EditorState {
        fn resume<'es, 'buf>(&'es mut self, buffer: &'buf mut Buffer) -> TempEditor<'es, 'buf> {
            TempEditor::new(self, buffer)
        }
    }

    pub struct TempEditor<'es, 'buf> {
        editor: Editor<'buf>,
        editor_state: &'es mut EditorState,
    }

    impl<'es, 'buf> TempEditor<'es, 'buf> {
        fn new(editor_state: &'es mut EditorState, buffer: &'buf mut Buffer) -> Self {
            let mut editor = Editor::new(buffer);
            if let Some(cursor) = editor_state.cursor {
                editor.set_cursor(cursor);
                editor.set_selection(editor_state.selection);
            }
            Self {
                editor,
                editor_state,
            }
        }

        pub fn with_editor_mut(mut self, func: impl FnOnce(&mut Editor)) -> Self {
            func(&mut self.editor);
            self.editor_state.cursor = Some(self.editor.cursor());
            self.editor_state.selection = self.editor.selection();
            self.editor_state.selection_bounds = self.editor.selection_bounds();
            self
        }
    }

    #[derive(Component, Clone, Copy, Debug)]
    pub struct CursorConfig {
        pub color: Color,
        pub width: f32,
    }

    impl Default for CursorConfig {
        fn default() -> Self {
            Self {
                color: Color::LinearRgba(LinearRgba::WHITE),
                width: 1.0,
            }
        }
    }

    #[derive(Component, Clone, Copy, Debug)]
    pub struct SelectionConfig {
        pub color: Color,
    }

    impl Default for SelectionConfig {
        fn default() -> Self {
            Self {
                color: Color::LinearRgba(LinearRgba::BLACK),
            }
        }
    }

    // TODO: does not support multiple windows
    #[derive(SystemParam)]
    pub struct HitSystemParams<'w, 's> {
        pub window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
        pub buffers:
            Query<'w, 's, (Entity, &'static CosmicBuffer, &'static GlobalTransform), With<Node>>,
    }

    pub struct HitOutput {
        pub entity: Entity,
        pub span_index: usize,
        pub position: Vec2,
    }

    /// Assumes only one entity gets hit (early returns)
    #[allow(clippy::type_complexity)]
    pub fn hit(params: HitSystemParams) -> Option<HitOutput> {
        let window = params.window.single();

        let cursor_window_position = window.cursor_position()?;

        for (entity, buffer, transform) in &params.buffers {
            let size = buffer.size();
            let size = Vec2::new(
                size.0.expect("Buffer has a width"),
                size.1.expect("Buffer has a height"),
            );
            let origin = transform.translation().truncate();
            let rect = Rect::from_center_size(origin, size);
            if rect.contains(cursor_window_position) {
                // top left corner of buffer (where +Y down, +X right)
                // TODO: slightly off for some reason, unsure if cosmic-text or this is wrong
                let offset = origin - size / 2.0;
                // position in buffer
                let position = cursor_window_position - offset;
                // TODO: fix the issue where this always registers a hit on the first span if no other is hit
                if let Some(text_cursor) = buffer.hit(position.x, position.y) {
                    // get attrs from cursor
                    let line = &buffer.lines[text_cursor.line];
                    let attrs = line.attrs_list().get_span(text_cursor.index);
                    let span_index = attrs.metadata;
                    // notify only the relevant child
                    return Some(HitOutput {
                        entity,
                        span_index,
                        position,
                    });
                }
            }
        }

        None
    }
}
