//! Shared serde helpers for the crate's hand-rolled bitfield newtypes.
//!
//! The bitfield newtypes (e.g. [`crate::map::TeleportFlags`]) are plain
//! integer wrappers with named flag constants rather than `bitflags` types, so
//! they need a small amount of custom serde glue to render readably. See
//! [`impl_bitfield_serde`].

/// The deserialization shape accepted for a bitfield newtype: either a bare
/// integer (the authoritative raw bit value) or an object carrying that value
/// in a `bits` field.
///
/// When serializing, a bitfield additionally emits a human-readable `flags`
/// array, which is ignored on the way back in — `bits` is always authoritative,
/// so the representation round-trips losslessly even for bits that have no
/// named flag.
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub(crate) enum BitfieldRepr<T> {
    /// a bare integer carrying the raw bit value
    Bits(T),
    /// an object carrying the raw bit value in its `bits` field (any
    /// accompanying `flags` array is ignored)
    Object {
        /// the authoritative raw bit value
        bits: T,
    },
}

impl<T> BitfieldRepr<T> {
    /// The raw bit value, regardless of which representation was supplied.
    pub(crate) fn into_bits(self) -> T {
        match self {
            Self::Bits(bits) | Self::Object { bits } => bits,
        }
    }
}

/// Implements [`serde::Serialize`] and [`serde::Deserialize`] for a hand-rolled
/// bitfield newtype `struct $t(pub $int)`.
///
/// Serialization emits `{ "bits": <raw>, "flags": [names of the set flags…] }`
/// — the raw value is authoritative and the `flags` array is a readable
/// annotation. Deserialization reads the authoritative `bits` value (from the
/// object) or a bare integer, so unknown bits survive a round trip.
///
/// Each `"NAME" => mask` pair maps a flag name to the mask (an expression of
/// type `$int`) whose bits must all be set for the name to be emitted.
macro_rules! impl_bitfield_serde {
    ($t:ty, $int:ty, $( $name:literal => $mask:expr ),+ $(,)?) => {
        impl serde::Serialize for $t {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::ser::SerializeStruct as _;
                let mut flags: Vec<&'static str> = Vec::new();
                $(
                    {
                        let mask: $int = $mask;
                        if mask != 0 && self.0 & mask == mask {
                            flags.push($name);
                        }
                    }
                )+
                let mut state = serializer.serialize_struct("Bitfield", 2)?;
                state.serialize_field("bits", &self.0)?;
                state.serialize_field("flags", &flags)?;
                state.end()
            }
        }

        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let repr = $crate::serde_helpers::BitfieldRepr::<$int>::deserialize(
                    deserializer,
                )?;
                Ok(Self(repr.into_bits()))
            }
        }
    };
}

pub(crate) use impl_bitfield_serde;

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    #[test]
    fn test_bitfield_serialises_bits_and_flags() -> Result<(), Box<dyn std::error::Error>> {
        // DEBIT (1 << 1) | TRIGGER_ANIMATION (1 << 4) == 18
        let permissions = crate::lsl::ScriptPermissions(
            crate::lsl::ScriptPermissions::DEBIT | crate::lsl::ScriptPermissions::TRIGGER_ANIMATION,
        );
        let json = serde_json::to_value(permissions)?;
        assert_eq!(
            json,
            serde_json::json!({ "bits": 18, "flags": ["DEBIT", "TRIGGER_ANIMATION"] })
        );
        Ok(())
    }

    #[test]
    fn test_bitfield_round_trips_including_unknown_bits() -> Result<(), Box<dyn std::error::Error>>
    {
        // A known flag plus a bit no name covers; `bits` keeps it lossless.
        let raw = crate::lsl::ScriptPermissions::DEBIT | (1 << 30);
        let permissions = crate::lsl::ScriptPermissions(raw);
        let json = serde_json::to_string(&permissions)?;
        let back: crate::lsl::ScriptPermissions = serde_json::from_str(&json)?;
        assert_eq!(back, permissions);
        // A bare integer is also accepted.
        let from_int: crate::lsl::ScriptPermissions = serde_json::from_str(&raw.to_string())?;
        assert_eq!(from_int, permissions);
        Ok(())
    }
}
