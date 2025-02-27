//! Types related to attachments

/// avatar attachment points
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum AvatarAttachmentPoint {
    /// Skull
    Skull,
    /// Nose
    Nose,
    /// Mouth
    Mouth,
    /// Tongue
    Tongue,
    /// Chin
    Chin,
    /// Jaw
    Jaw,
    /// Left Ear
    LeftEar,
    /// Right Ear
    RightEar,
    /// Alt Left Ear
    AltLeftEar,
    /// Alt Right Ear
    AltRightEar,
    /// Left Eye
    LeftEye,
    /// Right Eye
    RightEye,
    /// Alt Left Ear
    AltLeftEye,
    /// Alt Right Ear
    AltRightEye,
    /// Neck
    Neck,
    /// Left Shoulder
    LeftShoulder,
    /// Right Shoulder
    RightShoulder,
    /// L Upper Arm
    LeftUpperArm,
    /// R Upper Arm
    RightUpperArm,
    /// L Lower Arm
    LeftLowerArm,
    /// R Lower Arm
    RightLowerArm,
    /// Left Hand
    LeftHand,
    /// Right Hand
    RightHand,
    /// Left Ring Finger
    LeftRingFinger,
    /// Right Ring Finger
    RightRingFinger,
    /// Left Wing
    LeftWing,
    /// Right Wing
    RightWing,
    /// Chest
    Chest,
    /// Left Pec
    LeftPec,
    /// Right Pec
    RightPec,
    /// Stomach
    Stomach,
    /// Spine
    Spine,
    /// Tail Base
    TailBase,
    /// Tail Tip
    TailTip,
    /// Avatar Center
    AvatarCenter,
    /// Pelvis
    Pelvis,
    /// Groin
    Groin,
    /// Left Hip
    LeftHip,
    /// Right Hip
    RightHip,
    /// L Upper Leg
    LeftUpperLeg,
    /// R Upper Leg
    RightUpperLeg,
    /// L Lower Leg
    LeftLowerLeg,
    /// R Lower Leg
    RightLowerLeg,
    /// Left Foot
    LeftFoot,
    /// Right Foot
    RightFoot,
    /// Left Hind Foot
    LeftHindFoot,
    /// Right Hind Foot
    RightHindFoot,
}

impl std::fmt::Display for AvatarAttachmentPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AvatarAttachmentPoint::Skull => write!(f, "Skull"),
            AvatarAttachmentPoint::Nose => write!(f, "Nose"),
            AvatarAttachmentPoint::Mouth => write!(f, "Mouth"),
            AvatarAttachmentPoint::Tongue => write!(f, "Tongue"),
            AvatarAttachmentPoint::Chin => write!(f, "Chin"),
            AvatarAttachmentPoint::Jaw => write!(f, "Jaw"),
            AvatarAttachmentPoint::LeftEar => write!(f, "Left Ear"),
            AvatarAttachmentPoint::RightEar => write!(f, "Right Ear"),
            AvatarAttachmentPoint::AltLeftEar => write!(f, "Alt Left Ear"),
            AvatarAttachmentPoint::AltRightEar => write!(f, "Alt Right Ear"),
            AvatarAttachmentPoint::LeftEye => write!(f, "Left Eye"),
            AvatarAttachmentPoint::RightEye => write!(f, "Right Eye"),
            AvatarAttachmentPoint::AltLeftEye => write!(f, "Alt Left Eye"),
            AvatarAttachmentPoint::AltRightEye => write!(f, "Alt Right Eye"),
            AvatarAttachmentPoint::Neck => write!(f, "Neck"),
            AvatarAttachmentPoint::LeftShoulder => write!(f, "Left Shoulder"),
            AvatarAttachmentPoint::RightShoulder => write!(f, "Right Shoulder"),
            AvatarAttachmentPoint::LeftUpperArm => write!(f, "L Upper Arm"),
            AvatarAttachmentPoint::RightUpperArm => write!(f, "R Upper Arm"),
            AvatarAttachmentPoint::LeftLowerArm => write!(f, "L Lower Arm"),
            AvatarAttachmentPoint::RightLowerArm => write!(f, "R Lower Arm"),
            AvatarAttachmentPoint::LeftHand => write!(f, "Left Hand"),
            AvatarAttachmentPoint::RightHand => write!(f, "Right Hand"),
            AvatarAttachmentPoint::LeftRingFinger => write!(f, "Left Ring Finger"),
            AvatarAttachmentPoint::RightRingFinger => write!(f, "Right Ring Finger"),
            AvatarAttachmentPoint::LeftWing => write!(f, "Left Wing"),
            AvatarAttachmentPoint::RightWing => write!(f, "Right Wing"),
            AvatarAttachmentPoint::Chest => write!(f, "Chest"),
            AvatarAttachmentPoint::LeftPec => write!(f, "Left Pec"),
            AvatarAttachmentPoint::RightPec => write!(f, "Right Pec"),
            AvatarAttachmentPoint::Stomach => write!(f, "Stomach"),
            AvatarAttachmentPoint::Spine => write!(f, "Spine"),
            AvatarAttachmentPoint::TailBase => write!(f, "Tail Base"),
            AvatarAttachmentPoint::TailTip => write!(f, "Tail Tip"),
            AvatarAttachmentPoint::AvatarCenter => write!(f, "Avatar Center"),
            AvatarAttachmentPoint::Pelvis => write!(f, "Pelvis"),
            AvatarAttachmentPoint::Groin => write!(f, "Groin"),
            AvatarAttachmentPoint::LeftHip => write!(f, "Left Hip"),
            AvatarAttachmentPoint::RightHip => write!(f, "Right Hip"),
            AvatarAttachmentPoint::LeftUpperLeg => write!(f, "L Upper Leg"),
            AvatarAttachmentPoint::RightUpperLeg => write!(f, "R Upper Leg"),
            AvatarAttachmentPoint::LeftLowerLeg => write!(f, "L Lower Leg"),
            AvatarAttachmentPoint::RightLowerLeg => write!(f, "R Lower Leg"),
            AvatarAttachmentPoint::LeftFoot => write!(f, "Left Foot"),
            AvatarAttachmentPoint::RightFoot => write!(f, "Right Foot"),
            AvatarAttachmentPoint::LeftHindFoot => write!(f, "Left Hind Foot"),
            AvatarAttachmentPoint::RightHindFoot => write!(f, "Right Hind Foot"),
        }
    }
}

/// HUD attachment point
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum HudAttachmentPoint {
    /// HUD Center 2
    Center2,
    /// HUD Top Right
    TopRight,
    /// HUD Top
    Top,
    /// HUD Top Left
    TopLeft,
    /// HUD Center
    Center,
    /// HUD Bottom Left
    BottomLeft,
    /// HUD Bottom
    Bottom,
    /// HUT Bottom Right
    BottomRight,
}

impl std::fmt::Display for HudAttachmentPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HudAttachmentPoint::Center2 => write!(f, "HUD Center 2"),
            HudAttachmentPoint::TopRight => write!(f, "HUD Top Right"),
            HudAttachmentPoint::Top => write!(f, "HUD Top"),
            HudAttachmentPoint::TopLeft => write!(f, "HUD Top Left"),
            HudAttachmentPoint::Center => write!(f, "HUD Center"),
            HudAttachmentPoint::BottomLeft => write!(f, "HUD Bottom Left"),
            HudAttachmentPoint::Bottom => write!(f, "HUD Bottom"),
            HudAttachmentPoint::BottomRight => write!(f, "HUD Bottom Right"),
        }
    }
}

/// avatar and HUD attachment points
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum AttachmentPoint {
    /// avatar attachment point
    Avatar(AvatarAttachmentPoint),
    /// HUD attachment point
    Hud(HudAttachmentPoint),
}

impl std::fmt::Display for AttachmentPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttachmentPoint::Avatar(avatar_attachment_point) => {
                write!(f, "{}", avatar_attachment_point)
            }
            AttachmentPoint::Hud(hud_attachment_point) => write!(f, "{}", hud_attachment_point),
        }
    }
}

// TODO: FromStr instances
// TODO: parsers
