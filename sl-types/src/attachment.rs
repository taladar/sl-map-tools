//! Types related to attachments

#[cfg(feature = "chumsky")]
use chumsky::{
    Parser,
    prelude::{Simple, choice, just},
};

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
    #[must_use]
    pub const fn requires_bento(&self) -> bool {
        matches!(
            self,
            Self::Tongue
                | Self::AltLeftEar
                | Self::AltRightEar
                | Self::AltLeftEye
                | Self::AltRightEye
                | Self::LeftRingFinger
                | Self::RightRingFinger
                | Self::LeftWing
                | Self::RightWing
                | Self::TailBase
                | Self::TailTip
                | Self::Groin
                | Self::LeftHindFoot
                | Self::RightHindFoot
        )
    }
}

impl std::fmt::Display for AvatarAttachmentPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Skull => write!(f, "Skull"),
            Self::Nose => write!(f, "Nose"),
            Self::Mouth => write!(f, "Mouth"),
            Self::Tongue => write!(f, "Tongue"),
            Self::Chin => write!(f, "Chin"),
            Self::Jaw => write!(f, "Jaw"),
            Self::LeftEar => write!(f, "Left Ear"),
            Self::RightEar => write!(f, "Right Ear"),
            Self::AltLeftEar => write!(f, "Alt Left Ear"),
            Self::AltRightEar => write!(f, "Alt Right Ear"),
            Self::LeftEye => write!(f, "Left Eye"),
            Self::RightEye => write!(f, "Right Eye"),
            Self::AltLeftEye => write!(f, "Alt Left Eye"),
            Self::AltRightEye => write!(f, "Alt Right Eye"),
            Self::Neck => write!(f, "Neck"),
            Self::LeftShoulder => write!(f, "Left Shoulder"),
            Self::RightShoulder => write!(f, "Right Shoulder"),
            Self::LeftUpperArm => write!(f, "L Upper Arm"),
            Self::RightUpperArm => write!(f, "R Upper Arm"),
            Self::LeftLowerArm => write!(f, "L Lower Arm"),
            Self::RightLowerArm => write!(f, "R Lower Arm"),
            Self::LeftHand => write!(f, "Left Hand"),
            Self::RightHand => write!(f, "Right Hand"),
            Self::LeftRingFinger => write!(f, "Left Ring Finger"),
            Self::RightRingFinger => write!(f, "Right Ring Finger"),
            Self::LeftWing => write!(f, "Left Wing"),
            Self::RightWing => write!(f, "Right Wing"),
            Self::Chest => write!(f, "Chest"),
            Self::LeftPec => write!(f, "Left Pec"),
            Self::RightPec => write!(f, "Right Pec"),
            Self::Stomach => write!(f, "Stomach"),
            Self::Spine => write!(f, "Spine"),
            Self::TailBase => write!(f, "Tail Base"),
            Self::TailTip => write!(f, "Tail Tip"),
            Self::AvatarCenter => write!(f, "Avatar Center"),
            Self::Pelvis => write!(f, "Pelvis"),
            Self::Groin => write!(f, "Groin"),
            Self::LeftHip => write!(f, "Left Hip"),
            Self::RightHip => write!(f, "Right Hip"),
            Self::LeftUpperLeg => write!(f, "L Upper Leg"),
            Self::RightUpperLeg => write!(f, "R Upper Leg"),
            Self::LeftLowerLeg => write!(f, "L Lower Leg"),
            Self::RightLowerLeg => write!(f, "R Lower Leg"),
            Self::LeftFoot => write!(f, "Left Foot"),
            Self::RightFoot => write!(f, "Right Foot"),
            Self::LeftHindFoot => write!(f, "Left Hind Foot"),
            Self::RightHindFoot => write!(f, "Right Hind Foot"),
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
            "ATTACH_HEAD" | "Skull" | "head" => Ok(Self::Skull),
            "ATTACH_NOSE" | "Nose" | "nose" => Ok(Self::Nose),
            "ATTACH_MOUTH" | "Mouth" | "mouth" => Ok(Self::Mouth),
            "ATTACH_FACE_TONGUE" | "Tongue" | "tongue" => Ok(Self::Tongue),
            "ATTACH_CHIN" | "Chin" | "chin" => Ok(Self::Chin),
            "ATTACH_FACE_JAW" | "Jaw" | "jaw" => Ok(Self::Jaw),
            "ATTACH_LEAR" | "Left Ear" | "left ear" => Ok(Self::LeftEar),
            "ATTACH_REAR" | "Right Ear" | "right ear" => Ok(Self::RightEar),
            "ATTACH_FACE_LEAR" | "Alt Left Ear" | "left ear (extended)" => Ok(Self::AltLeftEar),
            "ATTACH_FACE_REAR" | "Alt Right Ear" | "right ear (extended)" => Ok(Self::AltRightEar),
            "ATTACH_LEYE" | "Left Eye" | "left eye" => Ok(Self::LeftEye),
            "ATTACH_REYE" | "Right Eye" | "right eye" => Ok(Self::RightEye),
            "ATTACH_FACE_LEYE" | "Alt Left Eye" | "left eye (extended)" => Ok(Self::AltLeftEye),
            "ATTACH_FACE_REYE" | "Alt Right Eye" | "right eye (extended)" => Ok(Self::AltRightEye),
            "ATTACH_NECK" | "Neck" | "neck" => Ok(Self::Neck),
            "ATTACH_LSHOULDER" | "Left Shoulder" | "left shoulder" => Ok(Self::LeftShoulder),
            "ATTACH_RSHOULDER" | "Right Shoulder" | "right shoulder" => Ok(Self::RightShoulder),
            "ATTACH_LUARM" | "L Upper Arm" | "left upper arm" => Ok(Self::LeftUpperArm),
            "ATTACH_RUARM" | "R Upper Arm" | "right upper arm" => Ok(Self::RightUpperArm),
            "ATTACH_LLARM" | "L Lower Arm" | "left lower arm" => Ok(Self::LeftLowerArm),
            "ATTACH_RLARM" | "R Lower Arm" | "right lower arm" => Ok(Self::RightLowerArm),
            "ATTACH_LHAND" | "Left Hand" | "left hand" => Ok(Self::LeftHand),
            "ATTACH_RHAND" | "Right Hand" | "right hand" => Ok(Self::RightHand),
            "ATTACH_LHAND_RING1" | "Left Ring Finger" | "left ring finger" => {
                Ok(Self::LeftRingFinger)
            }
            "ATTACH_RHAND_RING1" | "Right Ring Finger" | "right ring finger" => {
                Ok(Self::RightRingFinger)
            }
            "ATTACH_LWING" | "Left Wing" | "left wing" => Ok(Self::LeftWing),
            "ATTACH_RWING" | "Right Wing" | "right wing" => Ok(Self::RightWing),
            "ATTACH_CHEST" | "Chest" | "chest/sternum" | "chest" | "sternum" => Ok(Self::Chest),
            "ATTACH_LEFT_PEC" | "Left Pec" | "left pectoral" => Ok(Self::LeftPec),
            "ATTACH_RIGHT_PEC" | "Right Pec" | "right pectoral" => Ok(Self::RightPec),
            "ATTACH_BELLY" | "Stomach" | "belly/stomach/tummy" | "belly" | "stomach" | "tummy" => {
                Ok(Self::Stomach)
            }
            "ATTACH_BACK" | "Spine" | "back" => Ok(Self::Spine),
            "ATTACH_TAIL_BASE" | "Tail Base" | "tail base" => Ok(Self::TailBase),
            "ATTACH_TAIL_TIP" | "Tail Tip" | "tail tip" => Ok(Self::TailTip),
            "ATTACH_AVATAR_CENTER"
            | "Avatar Center"
            | "avatar center/root"
            | "avatar center"
            | "root" => Ok(Self::AvatarCenter),
            "ATTACH_PELVIS" | "Pelvis" | "pelvis" => Ok(Self::Pelvis),
            "ATTACH_GROIN" | "Groin" | "groin" => Ok(Self::Groin),
            "ATTACH_LHIP" | "Left Hip" | "left hip" => Ok(Self::LeftHip),
            "ATTACH_RHIP" | "Right Hip" | "right hip" => Ok(Self::RightHip),
            "ATTACH_LULEG" | "L Upper Leg" | "left upper leg" => Ok(Self::LeftUpperLeg),
            "ATTACH_RULEG" | "R Upper Leg" | "right upper leg" => Ok(Self::RightUpperLeg),
            "ATTACH_RLLEG" | "R Lower Leg" | "right lower leg" => Ok(Self::LeftLowerLeg),
            "ATTACH_LLLEG" | "L Lower Leg" | "left lower leg" => Ok(Self::RightLowerLeg),
            "ATTACH_LFOOT" | "Left Foot" | "left foot" => Ok(Self::LeftFoot),
            "ATTACH_RFOOT" | "Right Foot" | "right foot" => Ok(Self::RightFoot),
            "ATTACH_HIND_LFOOT" | "Left Hind Foot" | "left hind foot" => Ok(Self::LeftHindFoot),
            "ATTACH_HIND_RFOOT" | "Right Hind Foot" | "right hind foot" => Ok(Self::RightHindFoot),
            _ => Err(AvatarAttachmentPointParseError {
                value: s.to_string(),
            }),
        }
    }
}

/// parse an avatar attachment point
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn avatar_attachment_point_parser()
-> impl Parser<char, AvatarAttachmentPoint, Error = Simple<char>> {
    choice([
        just("ATTACH_HEAD")
            .or(just("Skull"))
            .or(just("head"))
            .to(AvatarAttachmentPoint::Skull)
            .boxed(),
        just("ATTACH_NOSE")
            .or(just("Nose"))
            .or(just("nose"))
            .to(AvatarAttachmentPoint::Nose)
            .boxed(),
        just("ATTACH_MOUTH")
            .or(just("Mouth"))
            .or(just("mouth"))
            .to(AvatarAttachmentPoint::Mouth)
            .boxed(),
        just("ATTACH_FACE_TONGUE")
            .or(just("Tongue"))
            .or(just("tongue"))
            .to(AvatarAttachmentPoint::Tongue)
            .boxed(),
        just("ATTACH_CHIN")
            .or(just("Chin"))
            .or(just("chin"))
            .to(AvatarAttachmentPoint::Chin)
            .boxed(),
        just("ATTACH_FACE_JAW")
            .or(just("Jaw"))
            .or(just("jaw"))
            .to(AvatarAttachmentPoint::Jaw)
            .boxed(),
        just("ATTACH_LEAR")
            .or(just("Left Ear"))
            .or(just("left ear"))
            .to(AvatarAttachmentPoint::LeftEar)
            .boxed(),
        just("ATTACH_REAR")
            .or(just("Right Ear"))
            .or(just("right ear"))
            .to(AvatarAttachmentPoint::RightEar)
            .boxed(),
        just("ATTACH_FACE_LEAR")
            .or(just("Alt Left Ear"))
            .or(just("left ear (extended)"))
            .to(AvatarAttachmentPoint::AltLeftEar)
            .boxed(),
        just("ATTACH_FACE_REAR")
            .or(just("Alt Right Ear"))
            .or(just("right ear (extended)"))
            .to(AvatarAttachmentPoint::AltRightEar)
            .boxed(),
        just("ATTACH_LEYE")
            .or(just("Left Eye"))
            .or(just("left eye"))
            .to(AvatarAttachmentPoint::LeftEye)
            .boxed(),
        just("ATTACH_REYE")
            .or(just("Right Eye"))
            .or(just("right eye"))
            .to(AvatarAttachmentPoint::RightEye)
            .boxed(),
        just("ATTACH_FACE_LEYE")
            .or(just("Alt Left Eye"))
            .or(just("left eye (extended)"))
            .to(AvatarAttachmentPoint::AltLeftEye)
            .boxed(),
        just("ATTACH_FACE_REYE")
            .or(just("Alt Right Eye"))
            .or(just("right eye (extended)"))
            .to(AvatarAttachmentPoint::AltRightEye)
            .boxed(),
        just("ATTACH_NECK")
            .or(just("Neck"))
            .or(just("neck"))
            .to(AvatarAttachmentPoint::Neck)
            .boxed(),
        just("ATTACH_LSHOULDER")
            .or(just("Left Shoulder"))
            .or(just("left shoulder"))
            .to(AvatarAttachmentPoint::LeftShoulder)
            .boxed(),
        just("ATTACH_RSHOULDER")
            .or(just("Right Shoulder"))
            .or(just("right shoulder"))
            .to(AvatarAttachmentPoint::RightShoulder)
            .boxed(),
        just("ATTACH_LUARM")
            .or(just("L Upper Arm"))
            .or(just("left upper arm"))
            .to(AvatarAttachmentPoint::LeftUpperArm)
            .boxed(),
        just("ATTACH_RUARM")
            .or(just("R Upper Arm"))
            .or(just("right upper arm"))
            .to(AvatarAttachmentPoint::RightUpperArm)
            .boxed(),
        just("ATTACH_LLARM")
            .or(just("L Lower Arm"))
            .or(just("left lower arm"))
            .to(AvatarAttachmentPoint::LeftLowerArm)
            .boxed(),
        just("ATTACH_RLARM")
            .or(just("R Lower Arm"))
            .or(just("right lower arm"))
            .to(AvatarAttachmentPoint::RightLowerArm)
            .boxed(),
        just("ATTACH_LHAND")
            .or(just("Left Hand"))
            .or(just("left hand"))
            .to(AvatarAttachmentPoint::LeftHand)
            .boxed(),
        just("ATTACH_RHAND")
            .or(just("Right Hand"))
            .or(just("right hand"))
            .to(AvatarAttachmentPoint::RightHand)
            .boxed(),
        just("ATTACH_LHAND_RING1")
            .or(just("Left Ring Finger"))
            .or(just("left ring finger"))
            .to(AvatarAttachmentPoint::LeftRingFinger)
            .boxed(),
        just("ATTACH_RHAND_RING1")
            .or(just("Right Ring Finger"))
            .or(just("right ring finger"))
            .to(AvatarAttachmentPoint::RightRingFinger)
            .boxed(),
        just("ATTACH_LWING")
            .or(just("Left Wing"))
            .or(just("left wing"))
            .to(AvatarAttachmentPoint::LeftWing)
            .boxed(),
        just("ATTACH_RWING")
            .or(just("Right Wing"))
            .or(just("right wing"))
            .to(AvatarAttachmentPoint::RightWing)
            .boxed(),
        just("ATTACH_CHEST")
            .or(just("Chest"))
            .or(just("chest/sternum"))
            .or(just("chest"))
            .or(just("sternum"))
            .to(AvatarAttachmentPoint::Chest)
            .boxed(),
        just("ATTACH_LEFT_PEC")
            .or(just("Left Pec"))
            .or(just("left pectoral"))
            .to(AvatarAttachmentPoint::LeftPec)
            .boxed(),
        just("ATTACH_RIGHT_PEC")
            .or(just("Right Pec"))
            .or(just("right pectoral"))
            .to(AvatarAttachmentPoint::RightPec)
            .boxed(),
        just("ATTACH_BELLY")
            .or(just("Stomach"))
            .or(just("belly/stomach/tummy"))
            .or(just("belly"))
            .or(just("stomach"))
            .or(just("tummy"))
            .to(AvatarAttachmentPoint::Stomach)
            .boxed(),
        just("ATTACH_BACK")
            .or(just("Spine"))
            .or(just("back"))
            .to(AvatarAttachmentPoint::Spine)
            .boxed(),
        just("ATTACH_TAIL_BASE")
            .or(just("Tail Base"))
            .or(just("tail base"))
            .to(AvatarAttachmentPoint::TailBase)
            .boxed(),
        just("ATTACH_TAIL_TIP")
            .or(just("Tail Tip"))
            .or(just("tail tip"))
            .to(AvatarAttachmentPoint::TailTip)
            .boxed(),
        just("ATTACH_AVATAR_CENTER")
            .or(just("Avatar Center"))
            .or(just("avatar center/root"))
            .or(just("avatar center"))
            .or(just("root"))
            .to(AvatarAttachmentPoint::AvatarCenter)
            .boxed(),
        just("ATTACH_PELVIS")
            .or(just("Pelvis"))
            .or(just("pelvis"))
            .to(AvatarAttachmentPoint::Pelvis)
            .boxed(),
        just("ATTACH_GROIN")
            .or(just("Groin"))
            .or(just("groin"))
            .to(AvatarAttachmentPoint::Groin)
            .boxed(),
        just("ATTACH_LHIP")
            .or(just("Left Hip"))
            .or(just("left hip"))
            .to(AvatarAttachmentPoint::LeftHip)
            .boxed(),
        just("ATTACH_RHIP")
            .or(just("Right Hip"))
            .or(just("right hip"))
            .to(AvatarAttachmentPoint::RightHip)
            .boxed(),
        just("ATTACH_LULEG")
            .or(just("L Upper Leg"))
            .or(just("left upper leg"))
            .to(AvatarAttachmentPoint::LeftUpperLeg)
            .boxed(),
        just("ATTACH_RULEG")
            .or(just("R Upper Leg"))
            .or(just("right upper leg"))
            .to(AvatarAttachmentPoint::RightUpperLeg)
            .boxed(),
        just("ATTACH_RLLEG")
            .or(just("R Lower Leg"))
            .or(just("right lower leg"))
            .to(AvatarAttachmentPoint::LeftLowerLeg)
            .boxed(),
        just("ATTACH_LLLEG")
            .or(just("L Lower Leg"))
            .or(just("left lower leg"))
            .to(AvatarAttachmentPoint::RightLowerLeg)
            .boxed(),
        just("ATTACH_LFOOT")
            .or(just("Left Foot"))
            .or(just("left foot"))
            .to(AvatarAttachmentPoint::LeftFoot)
            .boxed(),
        just("ATTACH_RFOOT")
            .or(just("Right Foot"))
            .or(just("right foot"))
            .to(AvatarAttachmentPoint::RightFoot)
            .boxed(),
        just("ATTACH_HIND_LFOOT")
            .or(just("Left Hind Foot"))
            .or(just("left hind foot"))
            .to(AvatarAttachmentPoint::LeftHindFoot)
            .boxed(),
        just("ATTACH_HIND_RFOOT")
            .or(just("Right Hind Foot"))
            .or(just("right hind foot"))
            .to(AvatarAttachmentPoint::RightHindFoot)
            .boxed(),
    ])
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
            Self::Center2 => write!(f, "HUD Center 2"),
            Self::TopRight => write!(f, "HUD Top Right"),
            Self::Top => write!(f, "HUD Top"),
            Self::TopLeft => write!(f, "HUD Top Left"),
            Self::Center => write!(f, "HUD Center"),
            Self::BottomLeft => write!(f, "HUD Bottom Left"),
            Self::Bottom => write!(f, "HUD Bottom"),
            Self::BottomRight => write!(f, "HUD Bottom Right"),
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
            "ATTACH_HUD_CENTER_2" | "HUD Center 2" | "Center 2" => Ok(Self::Center2),
            "ATTACH_HUD_TOP_RIGHT" | "HUD Top Right" | "Top Right" => Ok(Self::TopRight),
            "ATTACH_HUD_TOP_CENTER" | "HUD Top" | "Top" => Ok(Self::Top),
            "ATTACH_HUD_TOP_LEFT" | "HUD Top Left" | "Top Left" => Ok(Self::TopLeft),
            "ATTACH_HUD_CENTER_1" | "HUD Center" | "Center" => Ok(Self::Center),
            "ATTACH_HUD_BOTTOM_LEFT" | "HUD Bottom Left" | "Bottom Left" => Ok(Self::BottomLeft),
            "ATTACH_HUD_BOTTOM" | "HUD Bottom" | "Bottom" => Ok(Self::Bottom),
            "ATTACH_HUD_BOTTOM_RIGHT" | "HUD Bottom Right " | "Bottom Right" => {
                Ok(Self::BottomRight)
            }
            _ => Err(HudAttachmentPointParseError {
                value: s.to_string(),
            }),
        }
    }
}

/// parse a HUD attachment point
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
pub fn hud_attachment_point_parser() -> impl Parser<char, HudAttachmentPoint, Error = Simple<char>>
{
    choice([
        just("ATTACH_HUD_CENTER_2")
            .or(just("HUD Center 2"))
            .or(just("Center 2"))
            .to(HudAttachmentPoint::Center2),
        just("ATTACH_HUD_TOP_RIGHT")
            .or(just("HUD Top Right"))
            .or(just("Top Right"))
            .to(HudAttachmentPoint::TopRight),
        just("ATTACH_HUD_TOP_LEFT")
            .or(just("HUD Top Left"))
            .or(just("Top Left"))
            .to(HudAttachmentPoint::TopLeft),
        just("ATTACH_HUD_TOP_CENTER")
            .or(just("HUD Top"))
            .or(just("Top"))
            .to(HudAttachmentPoint::Top),
        just("ATTACH_HUD_CENTER_1")
            .or(just("HUD Center"))
            .or(just("Center"))
            .to(HudAttachmentPoint::Center),
        just("ATTACH_HUD_BOTTOM_LEFT")
            .or(just("HUD Bottom Left"))
            .or(just("Bottom Left"))
            .to(HudAttachmentPoint::BottomLeft),
        just("ATTACH_HUD_BOTTOM_RIGHT")
            .or(just("HUD Bottom Right "))
            .or(just("Bottom Right"))
            .to(HudAttachmentPoint::BottomRight),
        just("ATTACH_HUD_BOTTOM")
            .or(just("HUD Bottom"))
            .or(just("Bottom"))
            .to(HudAttachmentPoint::Bottom),
    ])
}

/// avatar and HUD attachment points
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the type is going to be used outside of the module"
)]
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
    /// <https://wiki.secondlife.com/wiki/Category:LSL_Attachment>
    ///
    #[must_use]
    pub fn from_repr(repr: usize) -> Option<Self> {
        AvatarAttachmentPoint::from_repr(repr)
            .map(Self::Avatar)
            .or_else(|| HudAttachmentPoint::from_repr(repr).map(Self::Hud))
    }
}

impl std::fmt::Display for AttachmentPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Avatar(avatar_attachment_point) => {
                write!(f, "{avatar_attachment_point}")
            }
            Self::Hud(hud_attachment_point) => write!(f, "{hud_attachment_point}"),
        }
    }
}

/// Error deserializing AttachmentPoint from String
#[derive(Debug, Clone)]
#[expect(
    clippy::module_name_repetitions,
    reason = "the error is going to be used outside of the module"
)]
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

/// parse an attachment point
///
/// # Errors
///
/// returns an error if the string could not be parsed
#[cfg(feature = "chumsky")]
#[must_use]
#[expect(
    clippy::module_name_repetitions,
    reason = "the parser is going to be used outside of the module"
)]
pub fn attachment_point_parser() -> impl Parser<char, AttachmentPoint, Error = Simple<char>> {
    avatar_attachment_point_parser()
        .map(AttachmentPoint::Avatar)
        .or(hud_attachment_point_parser().map(AttachmentPoint::Hud))
}

#[cfg(test)]
mod test {
    #[cfg(feature = "chumsky")]
    use super::{AttachmentPoint, HudAttachmentPoint, attachment_point_parser};
    #[cfg(feature = "chumsky")]
    use chumsky::Parser as _;
    #[cfg(feature = "chumsky")]
    use pretty_assertions::assert_eq;

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_parse_attachment_point_bottom_left() {
        assert_eq!(
            attachment_point_parser().parse("Bottom Left"),
            Ok(AttachmentPoint::Hud(HudAttachmentPoint::BottomLeft)),
        );
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_parse_attachment_point_bottom() {
        assert_eq!(
            attachment_point_parser().parse("Bottom"),
            Ok(AttachmentPoint::Hud(HudAttachmentPoint::Bottom)),
        );
    }

    #[cfg(feature = "chumsky")]
    #[test]
    fn test_parse_attachment_point_bottom_right() {
        assert_eq!(
            attachment_point_parser().parse("Bottom Right"),
            Ok(AttachmentPoint::Hud(HudAttachmentPoint::BottomRight)),
        );
    }
}
