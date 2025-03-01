//! Types related to attachments

/// avatar attachment points
#[derive(Debug, Clone, Hash, PartialEq, Eq, strum::FromRepr, strum::EnumIs)]
pub enum AvatarAttachmentPoint {
    /// Skull
    Skull = 2,
    /// Nose
    Nose = 17,
    /// Mouth
    Mouth = 11,
    /// Tongue
    Tongue = 52,
    /// Chin
    Chin = 12,
    /// Jaw
    Jaw = 47,
    /// Left Ear
    LeftEar = 13,
    /// Right Ear
    RightEar = 14,
    /// Alt Left Ear
    AltLeftEar = 48,
    /// Alt Right Ear
    AltRightEar = 49,
    /// Left Eye
    LeftEye = 15,
    /// Right Eye
    RightEye = 16,
    /// Alt Left Ear
    AltLeftEye = 50,
    /// Alt Right Ear
    AltRightEye = 51,
    /// Neck
    Neck = 39,
    /// Left Shoulder
    LeftShoulder = 3,
    /// Right Shoulder
    RightShoulder = 4,
    /// L Upper Arm
    LeftUpperArm = 20,
    /// R Upper Arm
    RightUpperArm = 18,
    /// L Lower Arm
    LeftLowerArm = 21,
    /// R Lower Arm
    RightLowerArm = 19,
    /// Left Hand
    LeftHand = 5,
    /// Right Hand
    RightHand = 6,
    /// Left Ring Finger
    LeftRingFinger = 41,
    /// Right Ring Finger
    RightRingFinger = 42,
    /// Left Wing
    LeftWing = 45,
    /// Right Wing
    RightWing = 46,
    /// Chest
    Chest = 1,
    /// Left Pec
    LeftPec = 29,
    /// Right Pec
    RightPec = 30,
    /// Stomach
    Stomach = 28,
    /// Spine
    Spine = 9,
    /// Tail Base
    TailBase = 43,
    /// Tail Tip
    TailTip = 44,
    /// Avatar Center
    AvatarCenter = 40,
    /// Pelvis
    Pelvis = 10,
    /// Groin
    Groin = 53,
    /// Left Hip
    LeftHip = 25,
    /// Right Hip
    RightHip = 22,
    /// L Upper Leg
    LeftUpperLeg = 26,
    /// R Upper Leg
    RightUpperLeg = 23,
    /// L Lower Leg
    LeftLowerLeg = 24,
    /// R Lower Leg
    RightLowerLeg = 27,
    /// Left Foot
    LeftFoot = 7,
    /// Right Foot
    RightFoot = 8,
    /// Left Hind Foot
    LeftHindFoot = 54,
    /// Right Hind Foot
    RightHindFoot = 55,
}

impl AvatarAttachmentPoint {
    /// returns true if the attachment point requires Bento
    pub fn requires_bento(&self) -> bool {
        match self {
            AvatarAttachmentPoint::Tongue => true,
            AvatarAttachmentPoint::AltLeftEar => true,
            AvatarAttachmentPoint::AltRightEar => true,
            AvatarAttachmentPoint::AltLeftEye => true,
            AvatarAttachmentPoint::AltRightEye => true,
            AvatarAttachmentPoint::LeftRingFinger => true,
            AvatarAttachmentPoint::RightRingFinger => true,
            AvatarAttachmentPoint::LeftWing => true,
            AvatarAttachmentPoint::RightWing => true,
            AvatarAttachmentPoint::TailBase => true,
            AvatarAttachmentPoint::TailTip => true,
            AvatarAttachmentPoint::Groin => true,
            AvatarAttachmentPoint::LeftHindFoot => true,
            AvatarAttachmentPoint::RightHindFoot => true,
            _ => false,
        }
    }
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

/// Error deserializing AvatarAttachmentPoint from String
#[derive(Debug, Clone)]
pub struct AvatarAttachmentPointParseError {
    /// the value that could not be parsed
    value: String,
}

impl std::fmt::Display for AvatarAttachmentPointParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Could not parse as AvatarAttachmentPoint: {}",
            self.value
        )
    }
}

impl std::str::FromStr for AvatarAttachmentPoint {
    type Err = AvatarAttachmentPointParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ATTACH_HEAD" | "Skull" | "head" => Ok(AvatarAttachmentPoint::Skull),
            "ATTACH_NOSE" | "Nose" | "nose" => Ok(AvatarAttachmentPoint::Nose),
            "ATTACH_MOUTH" | "Mouth" | "mouth" => Ok(AvatarAttachmentPoint::Mouth),
            "ATTACH_FACE_TONGUE" | "Tongue" | "tongue" => Ok(AvatarAttachmentPoint::Tongue),
            "ATTACH_CHIN" | "Chin" | "chin" => Ok(AvatarAttachmentPoint::Chin),
            "ATTACH_FACE_JAW" | "Jaw" | "jaw" => Ok(AvatarAttachmentPoint::Jaw),
            "ATTACH_LEAR" | "Left Ear" | "left ear" => Ok(AvatarAttachmentPoint::LeftEar),
            "ATTACH_REAR" | "Right Ear" | "right ear" => Ok(AvatarAttachmentPoint::RightEar),
            "ATTACH_FACE_LEAR" | "Alt Left Ear" | "left ear (extended)" => {
                Ok(AvatarAttachmentPoint::AltLeftEar)
            }
            "ATTACH_FACE_REAR" | "Alt Right Ear" | "right ear (extended)" => {
                Ok(AvatarAttachmentPoint::AltRightEar)
            }
            "ATTACH_LEYE" | "Left Eye" | "left eye" => Ok(AvatarAttachmentPoint::LeftEye),
            "ATTACH_REYE" | "Right Eye" | "right eye" => Ok(AvatarAttachmentPoint::RightEye),
            "ATTACH_FACE_LEYE" | "Alt Left Eye" | "left eye (extended)" => {
                Ok(AvatarAttachmentPoint::AltLeftEye)
            }
            "ATTACH_FACE_REYE" | "Alt Right Eye" | "right eye (extended)" => {
                Ok(AvatarAttachmentPoint::AltRightEye)
            }
            "ATTACH_NECK" | "Neck" | "neck" => Ok(AvatarAttachmentPoint::Neck),
            "ATTACH_LSHOULDER" | "Left Shoulder" | "left shoulder" => {
                Ok(AvatarAttachmentPoint::LeftShoulder)
            }
            "ATTACH_RSHOULDER" | "Right Shoulder" | "right shoulder" => {
                Ok(AvatarAttachmentPoint::RightShoulder)
            }
            "ATTACH_LUARM" | "L Upper Arm" | "left upper arm" => {
                Ok(AvatarAttachmentPoint::LeftUpperArm)
            }
            "ATTACH_RUARM" | "R Upper Arm" | "right upper arm" => {
                Ok(AvatarAttachmentPoint::RightUpperArm)
            }
            "ATTACH_LLARM" | "L Lower Arm" | "left lower arm" => {
                Ok(AvatarAttachmentPoint::LeftLowerArm)
            }
            "ATTACH_RLARM" | "R Lower Arm" | "right lower arm" => {
                Ok(AvatarAttachmentPoint::RightLowerArm)
            }
            "ATTACH_LHAND" | "Left Hand" | "left hand" => Ok(AvatarAttachmentPoint::LeftHand),
            "ATTACH_RHAND" | "Right Hand" | "right hand" => Ok(AvatarAttachmentPoint::RightHand),
            "ATTACH_LHAND_RING1" | "Left Ring Finger" | "left ring finger" => {
                Ok(AvatarAttachmentPoint::LeftRingFinger)
            }
            "ATTACH_RHAND_RING1" | "Right Ring Finger" | "right ring finger" => {
                Ok(AvatarAttachmentPoint::RightRingFinger)
            }
            "ATTACH_LWING" | "Left Wing" | "left wing" => Ok(AvatarAttachmentPoint::LeftWing),
            "ATTACH_RWING" | "Right Wing" | "right wing" => Ok(AvatarAttachmentPoint::RightWing),
            "ATTACH_CHEST" | "Chest" | "chest/sternum" | "chest" | "sternum" => {
                Ok(AvatarAttachmentPoint::Chest)
            }
            "ATTACH_LEFT_PEC" | "Left Pec" | "left pectoral" => Ok(AvatarAttachmentPoint::LeftPec),
            "ATTACH_RIGHT_PEC" | "Right Pec" | "right pectoral" => {
                Ok(AvatarAttachmentPoint::RightPec)
            }
            "ATTACH_BELLY" | "Stomach" | "belly/stomach/tummy" | "belly" | "stomach" | "tummy" => {
                Ok(AvatarAttachmentPoint::Stomach)
            }
            "ATTACH_BACK" | "Spine" | "back" => Ok(AvatarAttachmentPoint::Spine),
            "ATTACH_TAIL_BASE" | "Tail Base" | "tail base" => Ok(AvatarAttachmentPoint::TailBase),
            "ATTACH_TAIL_TIP" | "Tail Tip" | "tail tip" => Ok(AvatarAttachmentPoint::TailTip),
            "ATTACH_AVATAR_CENTER"
            | "Avatar Center"
            | "avatar center/root"
            | "avatar center"
            | "root" => Ok(AvatarAttachmentPoint::AvatarCenter),
            "ATTACH_PELVIS" | "Pelvis" | "pelvis" => Ok(AvatarAttachmentPoint::Pelvis),
            "ATTACH_GROIN" | "Groin" | "groin" => Ok(AvatarAttachmentPoint::Groin),
            "ATTACH_LHIP" | "Left Hip" | "left hip" => Ok(AvatarAttachmentPoint::LeftHip),
            "ATTACH_RHIP" | "Right Hip" | "right hip" => Ok(AvatarAttachmentPoint::RightHip),
            "ATTACH_LULEG" | "L Upper Leg" | "left upper leg" => {
                Ok(AvatarAttachmentPoint::LeftUpperLeg)
            }
            "ATTACH_RULEG" | "R Upper Leg" | "right upper leg" => {
                Ok(AvatarAttachmentPoint::RightUpperLeg)
            }
            "ATTACH_RLLEG" | "R Lower Leg" | "right lower leg" => {
                Ok(AvatarAttachmentPoint::LeftLowerLeg)
            }
            "ATTACH_LLLEG" | "L Lower Leg" | "left lower leg" => {
                Ok(AvatarAttachmentPoint::RightLowerLeg)
            }
            "ATTACH_LFOOT" | "Left Foot" | "left foot" => Ok(AvatarAttachmentPoint::LeftFoot),
            "ATTACH_RFOOT" | "Right Foot" | "right foot" => Ok(AvatarAttachmentPoint::RightFoot),
            "ATTACH_HIND_LFOOT" | "Left Hind Foot" | "left hind foot" => {
                Ok(AvatarAttachmentPoint::LeftHindFoot)
            }
            "ATTACH_HIND_RFOOT" | "Right Hind Foot" | "right hind foot" => {
                Ok(AvatarAttachmentPoint::RightHindFoot)
            }
            _ => Err(AvatarAttachmentPointParseError {
                value: s.to_string(),
            }),
        }
    }
}

/// HUD attachment point
#[derive(Debug, Clone, Hash, PartialEq, Eq, strum::FromRepr, strum::EnumIs)]
pub enum HudAttachmentPoint {
    /// HUD Center 2
    Center2 = 31,
    /// HUD Top Right
    TopRight = 32,
    /// HUD Top
    Top = 33,
    /// HUD Top Left
    TopLeft = 34,
    /// HUD Center
    Center = 35,
    /// HUD Bottom Left
    BottomLeft = 36,
    /// HUD Bottom
    Bottom = 37,
    /// HUT Bottom Right
    BottomRight = 38,
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

/// Error deserializing HudAttachmentPoint from String
#[derive(Debug, Clone)]
pub struct HudAttachmentPointParseError {
    /// the value that could not be parsed
    value: String,
}

impl std::fmt::Display for HudAttachmentPointParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse as HudAttachmentPoint: {}", self.value)
    }
}

impl std::str::FromStr for HudAttachmentPoint {
    type Err = HudAttachmentPointParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ATTACH_HUD_CENTER_2" | "HUD Center 2" => Ok(HudAttachmentPoint::Center2),
            "ATTACH_HUD_TOP_RIGHT" | "HUD Top Right" => Ok(HudAttachmentPoint::TopRight),
            "ATTACH_HUD_TOP_CENTER" | "HUD Top" => Ok(HudAttachmentPoint::Top),
            "ATTACH_HUD_TOP_LEFT" | "HUD Top Left" => Ok(HudAttachmentPoint::TopLeft),
            "ATTACH_HUD_CENTER_1" | "HUD Center" => Ok(HudAttachmentPoint::Center),
            "ATTACH_HUD_BOTTOM_LEFT" | "HUD Bottom Left" => Ok(HudAttachmentPoint::BottomLeft),
            "ATTACH_HUD_BOTTOM" | "HUD Bottom" => Ok(HudAttachmentPoint::Bottom),
            "ATTACH_HUD_BOTTOM_RIGHT" | "HUD Bottom Right " => Ok(HudAttachmentPoint::BottomRight),
            _ => Err(HudAttachmentPointParseError {
                value: s.to_string(),
            }),
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

impl AttachmentPoint {
    /// converts the numeric enum discriminant used in the LSL (and presumably
    /// C++) code for the attachment point into the respective enum variant
    ///
    /// https://wiki.secondlife.com/wiki/Category:LSL_Attachment
    ///
    pub fn from_repr(repr: usize) -> Option<AttachmentPoint> {
        if let Some(avatar_attachment_point) = AvatarAttachmentPoint::from_repr(repr) {
            Some(Self::Avatar(avatar_attachment_point))
        } else if let Some(hud_attachment_point) = HudAttachmentPoint::from_repr(repr) {
            Some(Self::Hud(hud_attachment_point))
        } else {
            None
        }
    }
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

/// Error deserializing AttachmentPoint from String
#[derive(Debug, Clone)]
pub struct AttachmentPointParseError {
    /// the value that could not be parsed
    value: String,
}

impl std::fmt::Display for AttachmentPointParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse as AttachmentPoint: {}", self.value)
    }
}

impl std::str::FromStr for AttachmentPoint {
    type Err = AttachmentPointParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(avatar_attachment_point) =
            <AvatarAttachmentPoint as std::str::FromStr>::from_str(s)
        {
            Ok(Self::Avatar(avatar_attachment_point))
        } else if let Ok(hud_attachment_point) =
            <HudAttachmentPoint as std::str::FromStr>::from_str(s)
        {
            Ok(Self::Hud(hud_attachment_point))
        } else {
            Err(AttachmentPointParseError {
                value: s.to_string(),
            })
        }
    }
}

// TODO: parsers
