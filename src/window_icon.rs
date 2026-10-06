//! The window and taskbar icon (`assets/ui/icon.png`, made by
//! `tools/gen_textures.py icon`): loaded like any image, then handed to the
//! window once it has arrived.

use bevy::prelude::*;
use bevy::winit::WinitWindows;

#[derive(Resource)]
struct IconImage(Handle<Image>);

pub struct WindowIconPlugin;

impl Plugin for WindowIconPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_icon).add_systems(Update, set_icon.run_if(resource_exists::<IconImage>));
    }
}

fn load_icon(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(IconImage(server.load("ui/icon.png")));
}

fn set_icon(mut commands: Commands, icon: Res<IconImage>, images: Res<Assets<Image>>, windows: NonSend<WinitWindows>) {
    let Some(image) = images.get(&icon.0) else { return };
    let (w, h) = (image.width(), image.height());
    let Some(data) = image.data.as_ref() else {
        commands.remove_resource::<IconImage>();
        return;
    };
    // (Only the full-size level: mipmaps may have been added after it.)
    let rgba = data.get(..(w * h * 4) as usize).map(<[u8]>::to_vec);
    match rgba.and_then(|px| winit::window::Icon::from_rgba(px, w, h).ok()) {
        Some(icon) => {
            for window in windows.windows.values() {
                window.set_window_icon(Some(icon.clone()));
            }
        }
        None => warn!("the window icon couldn't be used"),
    }
    commands.remove_resource::<IconImage>();
}
