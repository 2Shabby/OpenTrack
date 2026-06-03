use bevy::prelude::*;

pub(super) struct ButtonSpec {
    pub node: Node,
    pub font_size: f32,
    pub background: Color,
}

pub(super) fn button<Action: Component + Copy>(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: Action,
    spec: ButtonSpec,
) {
    parent
        .spawn((Button, spec.node, BackgroundColor(spec.background), action))
        .with_children(|parent| {
            parent.spawn((
                Text::new(label),
                TextFont {
                    font_size: spec.font_size,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}
