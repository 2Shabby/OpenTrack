mod assembly;
mod path;
mod types;
mod validation;

pub use assembly::generate_track_pieces;
pub use types::{
    GeneratedTrackInfo, PathFrame, RAIL_HEIGHT, RAIL_THICKNESS, SurfaceMix, TRACK_WIDTH,
    TrackBounds, TrackPiece, TrackPieceKind, TrackRecipe, car_spawn_for,
};
pub use validation::validate_track_pieces;
