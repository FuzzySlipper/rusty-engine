use gameplay_standard::ContinuousValue;
use serde::{de, Deserialize, Deserializer, Serializer};

pub fn serialize<S>(value: &ContinuousValue, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{:016x}", value.bits()))
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<ContinuousValue, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.len() != 16
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(de::Error::custom(
            "continuous bits must be 16 lowercase hexadecimal digits",
        ));
    }
    let bits = u64::from_str_radix(&value, 16).map_err(de::Error::custom)?;
    let continuous = ContinuousValue::from_bits(bits).map_err(de::Error::custom)?;
    if continuous.bits() != bits {
        return Err(de::Error::custom(
            "continuous bits must be normalized (negative zero is rejected)",
        ));
    }
    Ok(continuous)
}
