//! Editable forms for each record type.
//!
//! Each section entity implements [`FormEntity`], generated from one field list
//! by the `form_entity!` macro so field order, labels and value conversions
//! cannot drift apart. Form navigation and persistence are generic.

use goal_tracker_core::model;
use goal_tracker_core::rusqlite::Connection;
use goal_tracker_core::store::{self, Database, Identified, Record};
use goal_tracker_core::{Error, Result, history};

/// Description of one editable field.
pub(crate) struct Field {
    pub(crate) label: &'static str,
    pub(crate) optional: bool,
    pub(crate) choices: &'static [&'static str],
}

pub(crate) const fn field(label: &'static str) -> Field {
    Field {
        label,
        optional: false,
        choices: &[],
    }
}

pub(crate) const fn optional(label: &'static str) -> Field {
    Field {
        label,
        optional: true,
        choices: &[],
    }
}

pub(crate) const fn choices(label: &'static str, choices: &'static [&'static str]) -> Field {
    Field {
        label,
        optional: false,
        choices,
    }
}

/// One field's current value in an open form.
pub(crate) struct FormField {
    pub(crate) label: &'static str,
    pub(crate) value: String,
    pub(crate) optional: bool,
    pub(crate) choices: &'static [&'static str],
}

/// A record that can be created and edited through a text form.
pub(crate) trait FormEntity: Record + Identified {
    /// Editable fields, in display order.
    const FIELDS: &'static [Field];

    /// Current field values, in [`FormEntity::FIELDS`] order.
    fn values(&self) -> Vec<String>;

    /// Applies form values while preserving non-editable fields.
    fn set_values(&mut self, values: &[String]) -> Result<()>;

    /// A fresh record with a generated id and timestamp.
    fn blank() -> Self;

    /// Extra writes to run after an insert, inside the same transaction.
    fn after_insert(&self, _connection: &Connection) -> Result<()> {
        Ok(())
    }

    /// Extra writes to run after an update, inside the same transaction.
    fn after_update(&self, _connection: &Connection) -> Result<()> {
        Ok(())
    }
}

/// Converts a domain value to and from its single-line form representation.
pub(crate) trait FormValue: Sized {
    /// Renders the value.
    fn to_form(&self) -> String;

    /// Parses a form value, using `label` in error messages.
    fn from_form(value: &str, label: &'static str) -> Result<Self>;
}

impl FormValue for String {
    fn to_form(&self) -> String {
        self.clone()
    }

    fn from_form(value: &str, label: &'static str) -> Result<Self> {
        required(value, label)
    }
}

impl FormValue for Option<String> {
    fn to_form(&self) -> String {
        self.clone().unwrap_or_default()
    }

    fn from_form(value: &str, _label: &'static str) -> Result<Self> {
        Ok(optional_value(value))
    }
}

/// Skill levels are the only numeric form field; input is validated against
/// the domain range 0-`MAX_LEVEL`.
impl FormValue for i64 {
    fn to_form(&self) -> String {
        self.to_string()
    }

    fn from_form(value: &str, label: &'static str) -> Result<Self> {
        let parsed = value.parse::<i64>().ok();
        match parsed {
            Some(level) if (0..=model::MAX_LEVEL).contains(&level) => Ok(level),
            Some(_) | None => Err(Error::InvalidValue {
                kind: label,
                value: value.to_owned(),
            }),
        }
    }
}

macro_rules! enum_form_value {
    ($($name:ty),+ $(,)?) => {
        $(
            impl $crate::form::FormValue for $name {
                fn to_form(&self) -> ::std::string::String {
                    self.to_string()
                }

                fn from_form(
                    value: &str,
                    label: &'static str,
                ) -> ::goal_tracker_core::Result<Self> {
                    value.parse().map_err(|_error| {
                        ::goal_tracker_core::Error::InvalidValue {
                            kind: label,
                            value: value.to_owned(),
                        }
                    })
                }
            }
        )+
    };
}

enum_form_value!(
    model::GoalStatus,
    model::SkillStatus,
    model::MilestoneStatus,
    model::ProjectStatus,
    model::EvidenceType,
);

/// Generates [`FormEntity`] from a single field list.
///
/// Entries are `field: Type => descriptor`, in form order. `FIELDS`, `values`
/// and `set_values` are generated together, so they cannot disagree. The
/// optional `hooks` block may add extra trait items such as `after_update`.
macro_rules! form_entity {
    (
        $name:ty,
        blank: $blank:expr,
        fields: [$($field:ident : $ty:ty => $descriptor:expr),+ $(,)?]
        $(, hooks: { $($hook:item)* })?
        $(,)?
    ) => {
        impl $crate::form::FormEntity for $name {
            const FIELDS: &'static [$crate::form::Field] = &[$($descriptor),+];

            fn values(&self) -> ::std::vec::Vec<::std::string::String> {
                vec![$(<$ty as $crate::form::FormValue>::to_form(&self.$field)),+]
            }

            fn set_values(
                &mut self,
                values: &[::std::string::String],
            ) -> ::goal_tracker_core::Result<()> {
                if values.len() != Self::FIELDS.len() {
                    return ::std::result::Result::Err(
                        ::goal_tracker_core::Error::InvalidForm {
                            expected: Self::FIELDS.len(),
                            got: values.len(),
                        },
                    );
                }
                let mut form = values.iter().zip(Self::FIELDS.iter());
                $(
                    let (value, descriptor) = form
                        .next()
                        .ok_or(::goal_tracker_core::Error::InvalidForm {
                            expected: Self::FIELDS.len(),
                            got: values.len(),
                        })?;
                    self.$field =
                        <$ty as $crate::form::FormValue>::from_form(value, descriptor.label)?;
                )+
                ::std::result::Result::Ok(())
            }

            fn blank() -> Self {
                ($blank)()
            }

            $($($hook)*)?
        }
    };
}

/// Builds the form fields for `id`, or blank fields for a new record.
pub(crate) fn form_fields<E: FormEntity>(
    connection: &Connection,
    id: Option<&str>,
) -> Result<Vec<FormField>> {
    let values = match id {
        Some(id) => store::get::<E>(connection, id)?
            .ok_or_else(|| Error::NotFound(id.to_owned()))?
            .values(),
        None => E::blank().values(),
    };
    Ok(E::FIELDS
        .iter()
        .zip(values)
        .map(|(field, value)| FormField {
            label: field.label,
            value,
            optional: field.optional,
            choices: field.choices,
        })
        .collect())
}

/// Inserts (`id` is `None`) or updates a record from form values, returning its
/// id. The record and any history written by its hooks commit atomically.
pub(crate) fn save<E: FormEntity>(
    database: &mut Database,
    id: Option<&str>,
    values: &[String],
) -> Result<String> {
    database.with_transaction(|connection| {
        if let Some(id) = id {
            let mut entity =
                store::get::<E>(connection, id)?.ok_or_else(|| Error::NotFound(id.to_owned()))?;
            entity.set_values(values)?;
            store::update(connection, &entity)?;
            entity.after_update(connection)?;
            Ok(id.to_owned())
        } else {
            let mut entity = E::blank();
            entity.set_values(values)?;
            store::insert(connection, &entity)?;
            entity.after_insert(connection)?;
            Ok(entity.id().to_owned())
        }
    })
}

/// Requires a non-empty value.
pub(crate) fn required(value: &str, label: &'static str) -> Result<String> {
    if value.is_empty() {
        Err(Error::MissingField(label))
    } else {
        Ok(value.to_owned())
    }
}

/// Maps an empty value to `None`.
pub(crate) fn optional_value(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

const SKILL_LEVELS: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"];

form_entity! {
    model::Goal,
    blank: || model::Goal {
        id: model::new_id("goal"),
        title: String::new(),
        description: None,
        status: model::GoalStatus::Active,
        created_at: model::now(),
        archived_at: None,
    },
    fields: [
        title: String => field("Title"),
        description: Option<String> => optional("Description"),
        status: model::GoalStatus => choices("Status", model::GoalStatus::ALL),
    ],
    hooks: {
        fn after_insert(&self, connection: &Connection) -> Result<()> {
            history::revise_goal(connection, self)
        }

        fn after_update(&self, connection: &Connection) -> Result<()> {
            history::revise_goal(connection, self)
        }
    },
}

form_entity! {
    model::Skill,
    blank: || model::Skill {
        id: model::new_id("skill"),
        name: String::new(),
        description: None,
        status: model::SkillStatus::Active,
        level: 0,
        created_at: model::now(),
    },
    fields: [
        name: String => field("Name"),
        description: Option<String> => optional("Description"),
        status: model::SkillStatus => choices("Status", model::SkillStatus::ALL),
        level: i64 => choices("Level", SKILL_LEVELS),
    ],
}

form_entity! {
    model::Milestone,
    blank: || model::Milestone {
        id: model::new_id("milestone"),
        title: String::new(),
        description: None,
        status: model::MilestoneStatus::Planned,
        created_at: model::now(),
        completed_at: None,
        archived_at: None,
    },
    fields: [
        title: String => field("Title"),
        description: Option<String> => optional("Description"),
        status: model::MilestoneStatus => choices("Status", model::MilestoneStatus::ALL),
        completed_at: Option<String> => optional("Completed at"),
    ],
}

form_entity! {
    model::Evidence,
    blank: || model::Evidence {
        id: model::new_id("evidence"),
        title: String::new(),
        description: None,
        evidence_type: model::EvidenceType::Other,
        url: None,
        created_at: model::now(),
    },
    fields: [
        title: String => field("Title"),
        description: Option<String> => optional("Description"),
        evidence_type: model::EvidenceType => choices("Type", model::EvidenceType::ALL),
        url: Option<String> => optional("URL"),
    ],
}

form_entity! {
    model::Project,
    blank: || model::Project {
        id: model::new_id("project"),
        name: String::new(),
        description: None,
        repository_url: None,
        status: model::ProjectStatus::Active,
        created_at: model::now(),
        archived_at: None,
    },
    fields: [
        name: String => field("Name"),
        description: Option<String> => optional("Description"),
        repository_url: Option<String> => optional("Repository"),
        status: model::ProjectStatus => choices("Status", model::ProjectStatus::ALL),
    ],
}

form_entity! {
    model::Decision,
    blank: || model::Decision {
        id: model::new_id("decision"),
        description: String::new(),
        context: String::new(),
        decision: String::new(),
        rationale: String::new(),
        created_at: model::now(),
    },
    fields: [
        description: String => field("Description"),
        context: String => field("Context"),
        decision: String => field("Decision"),
        rationale: String => field("Rationale"),
    ],
}

#[cfg(test)]
mod tests {
    use super::FormEntity;
    use goal_tracker_core::model;

    /// A valid, distinct value for every field, in `FIELDS` order.
    fn sample_values<E: FormEntity>() -> Vec<String> {
        E::FIELDS
            .iter()
            .enumerate()
            .map(|(index, field)| {
                if let Some(choice) = field.choices.first() {
                    (*choice).to_owned()
                } else if field.optional {
                    String::new()
                } else {
                    format!("value-{index}")
                }
            })
            .collect()
    }

    fn assert_round_trip<E: FormEntity>() {
        let values = sample_values::<E>();

        let mut entity = E::blank();
        entity.set_values(&values).expect("valid sample values");
        assert_eq!(entity.values(), values);

        let mut restored = E::blank();
        restored
            .set_values(&entity.values())
            .expect("emitted values are valid");
        assert_eq!(restored.values(), entity.values());
    }

    #[test]
    fn every_entity_round_trips_its_values() {
        assert_round_trip::<model::Goal>();
        assert_round_trip::<model::Skill>();
        assert_round_trip::<model::Milestone>();
        assert_round_trip::<model::Evidence>();
        assert_round_trip::<model::Project>();
        assert_round_trip::<model::Decision>();
    }

    #[test]
    fn short_value_lists_are_rejected() {
        let mut goal = model::Goal::blank();
        let values = sample_values::<model::Goal>();
        assert!(goal.set_values(&values[..values.len() - 1]).is_err());
    }

    #[test]
    fn empty_required_fields_are_rejected() {
        let mut goal = model::Goal::blank();
        let values = vec![String::new(), String::new(), "active".to_owned()];
        assert!(goal.set_values(&values).is_err());
    }

    #[test]
    fn out_of_range_skill_levels_are_rejected() {
        let mut skill = model::Skill::blank();
        let mut values = sample_values::<model::Skill>();
        values[3] = "99".to_owned();
        assert!(skill.set_values(&values).is_err());
    }
}
