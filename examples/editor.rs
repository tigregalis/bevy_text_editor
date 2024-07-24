use bevy::prelude::*;
use bevy_text_editor::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TextEditorPlugin)
        .add_systems(Startup, setup)
        // .add_systems(Update, (animate_cursor, animate_selection))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    // let style = TextStyle {
    //     font_size: 70.0,
    //     color: Color::srgb(0.0, 1.0, 0.0),
    //     ..Default::default()
    // };

    // commands.spawn(TextBundle::from_sections([
    //     TextSection::new(
    //         "Hello, ",
    //         TextStyle {
    //             font_size: 40.0,
    //             color: Color::srgb(0.0, 1.0, 1.0),
    //             ..Default::default()
    //         },
    //     ),
    //     TextSection::new(
    //         "\n",
    //         TextStyle {
    //             font_size: 60.0,
    //             color: Color::srgb(1.0, 0.0, 0.0),
    //             ..Default::default()
    //         },
    //     ),
    //     TextSection::new(
    //         "World!\n",
    //         TextStyle {
    //             font_size: 60.0,
    //             color: Color::srgb(1.0, 0.0, 0.0),
    //             ..Default::default()
    //         },
    //     ),
    //     TextSection::new(
    //         "Hello, Bevy!\n",
    //         TextStyle {
    //             font_size: 50.0,
    //             color: Color::srgb(0.0, 1.0, 1.0),
    //             ..Default::default()
    //         },
    //     ),
    //     TextSection::new("and so on and so forth...", style),
    // ]));

    // Text with multiple sections
    commands.spawn((
        // Create a TextBundle that has a Text with a list of sections.
        TextEditorBundle::from_sections([
            TextSection::new(
                "Line 1\n",
                TextStyle {
                    font_size: 50.0,
                    ..default()
                },
            ),
            TextSection::new(
                "Line 2\n",
                TextStyle {
                    font_size: 50.0,
                    ..default()
                },
            ),
            TextSection::new(
                "\n",
                TextStyle {
                    font_size: 50.0,
                    ..default()
                },
            ),
            TextSection::new(
                "Line 4\n",
                TextStyle {
                    font_size: 50.0,
                    ..default()
                },
            ),
        ]),
    ));
}

// fn animate_cursor(mut query: Query<&mut CursorConfig>, time: Res<Time>) {
//     let seconds = time.elapsed_seconds();

//     for mut config in &mut query {
//         config.color = Color::srgb(
//             cycle(seconds, 0.5) * 0.5 + 0.5, // varies between 0.5 and 1
//             cycle(seconds, 1.1) * 0.5 + 0.5, // varies between 0.5 and 1
//             cycle(seconds, 1.7) * 0.5 + 0.5, // varies between 0.5 and 1
//         );
//         config.width = 2.0 + 8.0 * cycle(seconds, 8.0); // varies between 2 and 10
//     }
// }

// fn animate_selection(mut query: Query<&mut SelectionConfig>, time: Res<Time>) {
//     let seconds = time.elapsed_seconds();

//     for mut config in &mut query {
//         config.color = Color::srgb(
//             cycle(seconds, 0.9) * 0.5, // varies between 0 and 0.5
//             cycle(seconds, 1.5) * 0.5, // varies between 0 and 0.5
//             cycle(seconds, 1.9) * 0.5, // varies between 0 and 0.5
//         );
//     }
// }

// /// varies between 0 and 1
// fn cycle(seconds: f32, frequency: f32) -> f32 {
//     (seconds * frequency).sin() * 0.5 + 0.5
// }
