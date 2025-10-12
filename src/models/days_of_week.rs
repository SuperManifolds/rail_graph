use serde::{Deserialize, Serialize};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DaysOfWeek: u8 {
        const MONDAY    = 0b0000_0001;
        const TUESDAY   = 0b0000_0010;
        const WEDNESDAY = 0b0000_0100;
        const THURSDAY  = 0b0000_1000;
        const FRIDAY    = 0b0001_0000;
        const SATURDAY  = 0b0010_0000;
        const SUNDAY    = 0b0100_0000;
        const ALL_DAYS  = Self::MONDAY.bits() | Self::TUESDAY.bits() | Self::WEDNESDAY.bits()
                        | Self::THURSDAY.bits() | Self::FRIDAY.bits() | Self::SATURDAY.bits()
                        | Self::SUNDAY.bits();
        const WEEKDAYS  = Self::MONDAY.bits() | Self::TUESDAY.bits() | Self::WEDNESDAY.bits()
                        | Self::THURSDAY.bits() | Self::FRIDAY.bits();
        const WEEKENDS  = Self::SATURDAY.bits() | Self::SUNDAY.bits();
    }
}

impl Default for DaysOfWeek {
    fn default() -> Self {
        Self::ALL_DAYS
    }
}

impl DaysOfWeek {
    /// Check if all days are enabled
    #[must_use]
    pub const fn is_all_days(self) -> bool {
        self.bits() == Self::ALL_DAYS.bits()
    }

    /// Get a human-readable string representation
    #[must_use]
    pub fn to_display_string(self) -> String {
        if self.is_all_days() {
            return "All days".to_string();
        }
        if self == Self::WEEKDAYS {
            return "Weekdays".to_string();
        }
        if self == Self::WEEKENDS {
            return "Weekends".to_string();
        }

        let mut days = Vec::new();
        if self.contains(Self::MONDAY) { days.push("Mon"); }
        if self.contains(Self::TUESDAY) { days.push("Tue"); }
        if self.contains(Self::WEDNESDAY) { days.push("Wed"); }
        if self.contains(Self::THURSDAY) { days.push("Thu"); }
        if self.contains(Self::FRIDAY) { days.push("Fri"); }
        if self.contains(Self::SATURDAY) { days.push("Sat"); }
        if self.contains(Self::SUNDAY) { days.push("Sun"); }

        days.join(", ")
    }

    /// Get individual day from index (0 = Monday, 6 = Sunday)
    #[must_use]
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::MONDAY),
            1 => Some(Self::TUESDAY),
            2 => Some(Self::WEDNESDAY),
            3 => Some(Self::THURSDAY),
            4 => Some(Self::FRIDAY),
            5 => Some(Self::SATURDAY),
            6 => Some(Self::SUNDAY),
            _ => None,
        }
    }

    /// Get day name from index (0 = Monday, 6 = Sunday)
    #[must_use]
    pub const fn day_name(index: usize) -> Option<&'static str> {
        match index {
            0 => Some("Monday"),
            1 => Some("Tuesday"),
            2 => Some("Wednesday"),
            3 => Some("Thursday"),
            4 => Some("Friday"),
            5 => Some("Saturday"),
            6 => Some("Sunday"),
            _ => None,
        }
    }

    /// Get short day name from index (0 = Monday, 6 = Sunday)
    #[must_use]
    pub const fn day_short_name(index: usize) -> Option<&'static str> {
        match index {
            0 => Some("Mon"),
            1 => Some("Tue"),
            2 => Some("Wed"),
            3 => Some("Thu"),
            4 => Some("Fri"),
            5 => Some("Sat"),
            6 => Some("Sun"),
            _ => None,
        }
    }
}

// Custom serialization to store as u8
impl Serialize for DaysOfWeek {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.bits())
    }
}

impl<'de> Deserialize<'de> for DaysOfWeek {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = u8::deserialize(deserializer)?;
        Self::from_bits(bits).ok_or_else(|| serde::de::Error::custom("Invalid DaysOfWeek bits"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_all_days() {
        let days = DaysOfWeek::default();
        assert!(days.is_all_days());
        assert!(days.contains(DaysOfWeek::MONDAY));
        assert!(days.contains(DaysOfWeek::SUNDAY));
    }

    #[test]
    fn test_contains() {
        let days = DaysOfWeek::MONDAY | DaysOfWeek::FRIDAY;
        assert!(days.contains(DaysOfWeek::MONDAY));
        assert!(days.contains(DaysOfWeek::FRIDAY));
        assert!(!days.contains(DaysOfWeek::TUESDAY));
    }

    #[test]
    fn test_weekdays() {
        let days = DaysOfWeek::WEEKDAYS;
        assert!(days.contains(DaysOfWeek::MONDAY));
        assert!(days.contains(DaysOfWeek::FRIDAY));
        assert!(!days.contains(DaysOfWeek::SATURDAY));
        assert!(!days.contains(DaysOfWeek::SUNDAY));
    }

    #[test]
    fn test_weekends() {
        let days = DaysOfWeek::WEEKENDS;
        assert!(days.contains(DaysOfWeek::SATURDAY));
        assert!(days.contains(DaysOfWeek::SUNDAY));
        assert!(!days.contains(DaysOfWeek::MONDAY));
    }

    #[test]
    fn test_to_display_string() {
        assert_eq!(DaysOfWeek::ALL_DAYS.to_display_string(), "All days");
        assert_eq!(DaysOfWeek::WEEKDAYS.to_display_string(), "Weekdays");
        assert_eq!(DaysOfWeek::WEEKENDS.to_display_string(), "Weekends");

        let mon_wed = DaysOfWeek::MONDAY | DaysOfWeek::WEDNESDAY;
        assert_eq!(mon_wed.to_display_string(), "Mon, Wed");
    }

    #[test]
    fn test_from_index() {
        assert_eq!(DaysOfWeek::from_index(0), Some(DaysOfWeek::MONDAY));
        assert_eq!(DaysOfWeek::from_index(6), Some(DaysOfWeek::SUNDAY));
        assert_eq!(DaysOfWeek::from_index(7), None);
    }

    #[test]
    fn test_day_names() {
        assert_eq!(DaysOfWeek::day_name(0), Some("Monday"));
        assert_eq!(DaysOfWeek::day_name(6), Some("Sunday"));
        assert_eq!(DaysOfWeek::day_short_name(0), Some("Mon"));
        assert_eq!(DaysOfWeek::day_short_name(6), Some("Sun"));
    }

    #[test]
    fn test_serialization() {
        let days = DaysOfWeek::MONDAY | DaysOfWeek::FRIDAY;
        let serialized = serde_json::to_string(&days).expect("serialization should succeed");
        let deserialized: DaysOfWeek = serde_json::from_str(&serialized).expect("deserialization should succeed");
        assert_eq!(days, deserialized);
    }

    #[test]
    fn test_serialization_all_days() {
        let days = DaysOfWeek::ALL_DAYS;
        let serialized = serde_json::to_string(&days).expect("serialization should succeed");
        let deserialized: DaysOfWeek = serde_json::from_str(&serialized).expect("deserialization should succeed");
        assert_eq!(days, deserialized);
        assert!(deserialized.is_all_days());
    }
}
