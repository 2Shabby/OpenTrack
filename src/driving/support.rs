use bevy::prelude::*;

use super::{CarSpawn, model};
use crate::physics::{GroundContact, GroundSource, TrackPhysicsQueries};
use crate::surface::{SurfaceKind, SurfaceLibrary};

pub const CAR_GROUND_OFFSET: f32 = 0.05;
const WHEEL_SAMPLE_HALF_WIDTH: f32 = 0.82;
const WHEEL_SAMPLE_HALF_LENGTH: f32 = 1.72;
const SUPPORT_NORMAL_RESPONSE: f32 = 18.0;
const SUPPORT_POINT_RESPONSE: f32 = 24.0;
const WHEEL_CONTACT_COUNT: usize = model::WHEEL_COUNT;
const WHEEL_CONTACT_LABELS: [&str; WHEEL_CONTACT_COUNT] = ["FL", "FR", "RL", "RR"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VehicleSupportState {
    Grounded,
    Partial,
    Airborne,
}

impl VehicleSupportState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Grounded => "grounded",
            Self::Partial => "partial",
            Self::Airborne => "airborne",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VehicleSupportFrame {
    state: VehicleSupportState,
    pub contact_count: u8,
    pub point: Vec3,
    pub normal: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub rotation: Quat,
    pub surface: SurfaceKind,
    pub boost_direction: Option<Vec3>,
}

impl Default for VehicleSupportFrame {
    fn default() -> Self {
        Self::new(
            VehicleSupportState::Grounded,
            4,
            Vec3::ZERO,
            Vec3::Y,
            0.0,
            SurfaceKind::Asphalt,
            None,
        )
    }
}

impl VehicleSupportFrame {
    pub(super) fn from_spawn(spawn: CarSpawn) -> Self {
        Self::new(
            VehicleSupportState::Grounded,
            4,
            spawn.translation - spawn.up.normalize_or(Vec3::Y) * CAR_GROUND_OFFSET,
            spawn.up,
            spawn.yaw,
            SurfaceKind::Asphalt,
            None,
        )
    }

    pub(super) fn from_contacts(
        yaw: f32,
        position: Vec3,
        contacts: WheelContacts,
        center_ground: GroundContact,
        previous: Self,
    ) -> Self {
        let road_contacts = contacts.road_contacts();
        let contact_count = road_contacts.len();

        if contact_count == 0 && center_ground.source == GroundSource::OffTrack {
            return Self::new(
                VehicleSupportState::Airborne,
                0,
                position - previous.normal * CAR_GROUND_OFFSET,
                previous.normal,
                yaw,
                previous.surface,
                previous.boost_direction,
            );
        }

        let state = match contact_count {
            2.. => VehicleSupportState::Grounded,
            _ => VehicleSupportState::Partial,
        };
        let point = averaged_contact_point(&road_contacts).unwrap_or(center_ground.point);
        let normal = averaged_contact_normal(&road_contacts).unwrap_or(center_ground.normal);
        let surface = dominant_surface(&road_contacts).unwrap_or(center_ground.surface);
        let boost_direction = road_contacts
            .iter()
            .find_map(|contact| contact.boost_direction)
            .or(center_ground.boost_direction);

        Self::new(
            state,
            contact_count as u8,
            point,
            normal,
            yaw,
            surface,
            boost_direction,
        )
    }

    pub(super) fn resolved_towards(self, target: Self, yaw: f32, dt: f32) -> Self {
        if target.state == VehicleSupportState::Airborne {
            return Self::new(
                target.state,
                target.contact_count,
                target.point,
                self.normal,
                yaw,
                target.surface,
                target.boost_direction,
            );
        }

        let normal_blend = 1.0 - (-SUPPORT_NORMAL_RESPONSE * dt.max(0.0)).exp();
        let point_blend = 1.0 - (-SUPPORT_POINT_RESPONSE * dt.max(0.0)).exp();
        let normal = self
            .normal
            .lerp(target.normal, normal_blend)
            .normalize_or(target.normal);
        let point = self.point.lerp(target.point, point_blend);

        Self::new(
            target.state,
            target.contact_count,
            point,
            normal,
            yaw,
            target.surface,
            target.boost_direction,
        )
    }

    pub(super) fn supported_center(self, position: Vec3) -> Vec3 {
        if self.state == VehicleSupportState::Airborne {
            return position;
        }

        let distance = (position - self.point).dot(self.normal);
        position + self.normal * (CAR_GROUND_OFFSET - distance)
    }

    pub(super) fn ground_source(self, center_ground: GroundContact) -> GroundSource {
        if self.state == VehicleSupportState::Airborne {
            center_ground.source
        } else {
            GroundSource::Road
        }
    }

    pub fn state_label(self) -> &'static str {
        self.state.label()
    }

    fn new(
        state: VehicleSupportState,
        contact_count: u8,
        point: Vec3,
        normal: Vec3,
        yaw: f32,
        surface: SurfaceKind,
        boost_direction: Option<Vec3>,
    ) -> Self {
        let normal = normal.normalize_or(Vec3::Y);
        let rotation = crate::geometry::rotation_from_yaw_and_up(yaw, normal);

        Self {
            state,
            contact_count,
            point,
            normal,
            forward: rotation * Vec3::Z,
            right: rotation * Vec3::X,
            rotation,
            surface,
            boost_direction,
        }
    }
}

#[derive(Clone, Copy)]
pub struct WheelContacts {
    contacts: [GroundContact; WHEEL_CONTACT_COUNT],
}

impl Default for WheelContacts {
    fn default() -> Self {
        let contact = GroundContact {
            source: GroundSource::Road,
            surface: SurfaceKind::Asphalt,
            boost_direction: None,
            point: Vec3::ZERO,
            normal: Vec3::Y,
        };
        Self {
            contacts: [contact; WHEEL_CONTACT_COUNT],
        }
    }
}

impl WheelContacts {
    pub(super) fn sample(
        physics: &impl TrackPhysicsQueries,
        center: Vec3,
        basis: &model::MotionBasis,
    ) -> Self {
        let front = basis.forward * WHEEL_SAMPLE_HALF_LENGTH;
        let rear = -basis.forward * WHEEL_SAMPLE_HALF_LENGTH;
        let left = -basis.right * WHEEL_SAMPLE_HALF_WIDTH;
        let right = basis.right * WHEEL_SAMPLE_HALF_WIDTH;

        Self {
            contacts: [
                physics.ground_at(center + front + left),
                physics.ground_at(center + front + right),
                physics.ground_at(center + rear + left),
                physics.ground_at(center + rear + right),
            ],
        }
    }

    pub fn summary(self) -> String {
        WHEEL_CONTACT_LABELS
            .iter()
            .zip(self.contacts)
            .map(|(label, contact)| format!("{label}:{}", contact.label()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn friction(self, surfaces: &SurfaceLibrary) -> model::SurfaceFriction {
        model::SurfaceFriction {
            wheels: self.contacts.map(|contact| {
                let surface = surfaces.get(contact.surface);
                model::WheelFriction {
                    longitudinal: surface.longitudinal_friction,
                    lateral: surface.lateral_friction,
                }
            }),
        }
    }

    pub fn split_surface(self) -> bool {
        self.contacts[1..].iter().any(|contact| {
            contact.surface != self.contacts[0].surface || contact.source != self.contacts[0].source
        })
    }

    fn road_contacts(self) -> Vec<GroundContact> {
        self.contacts
            .into_iter()
            .filter(|contact| contact.source == GroundSource::Road)
            .collect()
    }
}

impl GroundContact {
    pub(super) fn label(self) -> String {
        format!("{}:{}", self.source.label(), self.surface.label())
    }
}

pub(super) struct SupportSample {
    pub center_ground: GroundContact,
    pub contacts: WheelContacts,
    pub target: VehicleSupportFrame,
}

impl SupportSample {
    pub(super) fn is_airborne_offtrack(&self) -> bool {
        self.target.state == VehicleSupportState::Airborne
            && self.center_ground.source == GroundSource::OffTrack
    }
}

pub(super) fn sample_vehicle_support(
    physics: &impl TrackPhysicsQueries,
    yaw: f32,
    position: Vec3,
    velocity: Vec3,
    basis_normal: Vec3,
    previous: VehicleSupportFrame,
) -> SupportSample {
    let center_ground = physics.ground_at(position);
    let basis = model::MotionBasis::from_ground(yaw, basis_normal, velocity);
    let contacts = WheelContacts::sample(physics, position, &basis);
    let target =
        VehicleSupportFrame::from_contacts(yaw, position, contacts, center_ground, previous);

    SupportSample {
        center_ground,
        contacts,
        target,
    }
}

fn averaged_contact_point(contacts: &[GroundContact]) -> Option<Vec3> {
    (!contacts.is_empty())
        .then(|| contacts.iter().map(|contact| contact.point).sum::<Vec3>() / contacts.len() as f32)
}

fn averaged_contact_normal(contacts: &[GroundContact]) -> Option<Vec3> {
    (!contacts.is_empty()).then(|| {
        contacts
            .iter()
            .map(|contact| contact.normal)
            .sum::<Vec3>()
            .normalize_or(Vec3::Y)
    })
}

fn dominant_surface(contacts: &[GroundContact]) -> Option<SurfaceKind> {
    let mut counts = [(SurfaceKind::Asphalt, 0u8); 4];
    counts[1].0 = SurfaceKind::Dirt;
    counts[2].0 = SurfaceKind::Ice;
    counts[3].0 = SurfaceKind::Boost;

    for contact in contacts {
        if let Some((_, count)) = counts
            .iter_mut()
            .find(|(surface, _)| *surface == contact.surface)
        {
            *count = count.saturating_add(1);
        }
    }

    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .and_then(|(surface, count)| (count > 0).then_some(surface))
}
