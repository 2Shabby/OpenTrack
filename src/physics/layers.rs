use avian3d::prelude::{LayerMask, SpatialQueryFilter};

pub const TRACK_ROAD_LAYER: LayerMask = LayerMask(1 << 0);
pub const TRACK_RAIL_LAYER: LayerMask = LayerMask(1 << 1);
pub const VEHICLE_LAYER: LayerMask = LayerMask(1 << 2);

pub fn road_query_filter() -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask(TRACK_ROAD_LAYER)
}

pub fn rail_query_filter() -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask(TRACK_RAIL_LAYER)
}
