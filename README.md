# `bevy_text_editor`

```sh
cargo run --example editor
```

VERY WIP

Basic idea:

1. Construct an ephemeral `TempEditor` (wraps `cosmic_text::Editor`) around the existing `cosmic_text::Buffer`,
2. Apply any changes to the `TempEditor` and it will be reflected in the `Buffer`
3. Extract and store any state that needs to be persisted between frames in EditorState (Cursor, Selection) and drop the `TempEditor`
4. Working backwards, update the `Text` component from the updated `Buffer`

TODO:

- [x] when cursor is at 0 on any line, it doesn't insert anything... why?
- [x] when I empty the buffer all hell breaks loose
- [x] when I try to backspace from the start of a line, sometimes everything blows up
- [x] selections
- [ ] the cursor should be its own entity! (and there should be the possibility of multiple cursors)
- [ ] multiple windows
- [ ] "Focused" Editor, not every editor
- [ ] "external"/programmatic changes to the text/spans should update the cursor/selection safely
- [ ] currently text spans have been cut out of this implementation
- [ ] with spans-as-entities (not yet implemented) it should be possible to restrict editing (e.g. only edit a span)
- [x] mouse click handling
- [ ] mouse drag handling
- [ ] multi-click handling is a little bit broken...
      (sometimes loses clicks, sometimes over-clicks... this might be a bevy one frame delay thing)
      maybe this implementation is better? https://devblogs.microsoft.com/oldnewthing/20041018-00/?p=37543
- [ ] selections are a little bit broken (or is this just a feature?): multi-click changes the selection "mode"
- [ ] shift/ctrl/alt/esc handling
- [ ] the selection should be its own entity! (and there should be the possibility of multiple selections)
