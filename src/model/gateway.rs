//! Models pertaining to the gateway.

use parking_lot::RwLock;
use serde::de::{self, DeserializeSeed, MapAccess};
use serde::ser::{SerializeStruct, Serialize, Serializer};
use serde_repr::{Serialize_repr, Deserialize_repr};
use std::{fmt, sync::Arc};
use super::utils::*;
use super::prelude::*;
use bitflags::bitflags;

/// A representation of the data retrieved from the bot gateway endpoint.
///
/// This is different from the [`Gateway`], as this includes the number of
/// shards that Discord recommends to use for a bot user.
///
/// This is only applicable to bot users.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BotGateway {
    /// Information describing how many gateway sessions you can initiate within
    /// a ratelimit period.
    pub session_start_limit: SessionStartLimit,
    /// The number of shards that is recommended to be used by the current bot
    /// user.
    pub shards: u64,
    /// The gateway to connect to.
    pub url: String,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

/// Representation of an activity that a [`User`] is performing.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Activity {
    /// The ID of the application for the activity.
    pub application_id: Option<ApplicationId>,
    /// Images for the presence and their texts.
    pub assets: Option<ActivityAssets>,
    /// What the user is doing.
    pub details: Option<String>,
    /// Activity flags describing what the payload includes.
    pub flags: Option<ActivityFlags>,
    /// Whether or not the activity is an instanced game session.
    pub instance: Option<bool>,
    /// The type of activity being performed
    #[serde(default = "ActivityType::default", rename = "type")]
    pub kind: ActivityType,
    /// The name of the activity.
    pub name: String,
    /// Information about the user's current party.
    pub party: Option<ActivityParty>,
    /// Secrets for Rich Presence joining and spectating.
    pub secrets: Option<ActivitySecrets>,
    /// The user's current party status.
    pub state: Option<String>,
    /// Unix timestamps for the start and/or end times of the activity.
    pub timestamps: Option<ActivityTimestamps>,
    /// The Stream URL if [`kind`] is [`ActivityType::Streaming`].
    ///
    /// [`ActivityType::Streaming`]: enum.ActivityType.html#variant.Streaming
    /// [`kind`]: #structfield.kind
    pub url: Option<String>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "model")]
impl Activity {
    /// Creates a `Game` struct that appears as a `Playing <name>` status.
    ///
    /// **Note**: Maximum `name` length is 128.
    ///
    /// # Examples
    ///
    /// Create a command that sets the current activity:
    ///
    /// ```rust,no_run
    /// use serenity::model::gateway::Activity;
    /// use serenity::model::channel::Message;
    /// # #[cfg(feature = "framework")]
    /// use serenity::framework::standard::{Args, CommandResult, macros::command};
    /// # #[cfg(feature = "client")]
    /// use serenity::client::Context;
    ///
    /// # #[cfg(feature = "framework")]
    /// #[command]
    /// fn activity(ctx: &mut Context, _msg: &Message, args: Args) -> CommandResult {
    ///     let name = args.message();
    ///     ctx.set_activity(Activity::playing(&name));
    ///
    ///     Ok(())
    /// }
    /// #
    /// # fn main() {}
    /// ```
    pub fn playing(name: &str) -> Activity {
        Activity {
            application_id: None,
            assets: None,
            details: None,
            flags: None,
            instance: None,
            kind: ActivityType::Playing,
            name: name.to_string(),
            party: None,
            secrets: None,
            state: None,
            timestamps: None,
            url: None,
            _nonexhaustive: (),
        }
    }

    /// Creates an `Activity` struct that appears as a `Streaming <name>`
    /// status.
    ///
    /// **Note**: Maximum `name` length is 128.
    ///
    /// # Examples
    ///
    /// Create a command that sets the current streaming status:
    ///
    /// ```rust,no_run
    /// use serenity::model::gateway::Activity;
    /// use serenity::model::channel::Message;
    /// # #[cfg(feature = "framework")]
    /// use serenity::framework::standard::{Args, CommandResult, macros::command};
    /// # #[cfg(feature = "client")]
    /// use serenity::client::Context;
    ///
    /// # #[cfg(feature = "framework")]
    /// #[command]
    /// fn stream(ctx: &mut Context, _msg: &Message, args: Args) -> CommandResult {
    ///     const STREAM_URL: &str = "...";
    ///
    ///     let name = args.message();
    ///     ctx.set_activity(Activity::streaming(&name, STREAM_URL));
    ///
    ///     Ok(())
    /// }
    /// #
    /// # fn main() {}
    /// ```
    pub fn streaming(name: &str, url: &str) -> Activity {
        Activity {
            application_id: None,
            assets: None,
            details: None,
            flags: None,
            instance: None,
            kind: ActivityType::Streaming,
            name: name.to_string(),
            party: None,
            secrets: None,
            state: None,
            timestamps: None,
            url: Some(url.to_string()),
            _nonexhaustive: (),
        }
    }

    /// Creates a `Game` struct that appears as a `Listening to <name>` status.
    ///
    /// **Note**: Maximum `name` length is 128.
    ///
    /// # Examples
    ///
    /// Create a command that sets the current listening status:
    ///
    /// ```rust,no_run
    /// use serenity::model::gateway::Activity;
    /// use serenity::model::channel::Message;
    /// # #[cfg(feature = "framework")]
    /// use serenity::framework::standard::{Args, CommandResult, macros::command};
    /// # #[cfg(feature = "client")]
    /// use serenity::client::Context;
    ///
    /// # #[cfg(feature = "framework")]
    /// #[command]
    /// fn listen(ctx: &mut Context, _msg: &Message, args: Args) -> CommandResult {
    ///     let name = args.message();
    ///     ctx.set_activity(Activity::listening(&name));
    ///
    ///     Ok(())
    /// }
    /// #
    /// # fn main() {}
    /// ```
    pub fn listening(name: &str) -> Activity {
        Activity {
            application_id: None,
            assets: None,
            details: None,
            flags: None,
            instance: None,
            kind: ActivityType::Listening,
            name: name.to_string(),
            party: None,
            secrets: None,
            state: None,
            timestamps: None,
            url: None,
            _nonexhaustive: (),
        }
    }
}

/// The assets for an activity.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityAssets {
    /// The ID for a large asset of the activity, usually a snowflake.
    pub large_image: Option<String>,
    /// Text displayed when hovering over the large image of the activity.
    pub large_text: Option<String>,
    /// The ID for a small asset of the activity, usually a snowflake.
    pub small_image: Option<String>,
    /// Text displayed when hovering over the small image of the activity.
    pub small_text: Option<String>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

bitflags! {
    /// A set of flags defining what is in an activity's payload.
    #[derive(Deserialize, Serialize)]
    pub struct ActivityFlags: u64 {
        /// Whether the activity is an instance activity.
        const INSTANCE = 0b001;
        /// Whether the activity is joinable.
        const JOIN = 0b010;
        /// Whether the activity can be spectated.
        const SPECTATE = 0b011;
        /// Whether a request can be sent to join the user's party.
        const JOIN_REQUEST = 0b100;
        /// Whether the activity can be synced.
        const SYNC = 0b101;
        /// Whether the activity can be played.
        const PLAY = 0b110;
    }
}

/// Information about an activity's party.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityParty {
    /// The ID of the party.
    pub id: Option<String>,
    /// Used to show the party's current and maximum size.
    pub size: Option<[u64; 2]>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

/// Secrets for an activity.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivitySecrets {
    /// The secret for joining a party.
    pub join: Option<String>,
    /// The secret for a specific instanced match.
    #[serde(rename = "match")]
    pub match_: Option<String>,
    /// The secret for spectating an activity.
    pub spectate: Option<String>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Copy, Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ActivityType {
    /// An indicator that the user is playing a game.
    Playing = 0,
    /// An indicator that the user is streaming to a service.
    Streaming = 1,
    /// An indicator that the user is listening to something.
    Listening = 2,
    #[doc(hidden)]
    __Nonexhaustive,
}

impl Default for ActivityType {
    fn default() -> Self { ActivityType::Playing }
}

/// A representation of the data retrieved from the gateway endpoint.
///
/// For the bot-specific gateway, refer to [`BotGateway`].
///
/// [`BotGateway`]: struct.BotGateway.html
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Gateway {
    /// The gateway to connect to.
    pub url: String,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

/// Information detailing the current online status of a [`User`].
///
/// [`User`]: ../user/struct.User.html
#[derive(Clone, Debug)]
pub struct Presence {
    /// The activity that a [`User`] is performing.
    ///
    /// [`User`]: struct.User.html
    pub activity: Option<Activity>,
    /// The date of the last presence update.
    pub last_modified: Option<u64>,
    /// The nickname of the member, if applicable.
    pub nick: Option<String>,
    /// The user's online status.
    pub status: OnlineStatus,
    /// The Id of the [`User`](../user/struct.User.html). Can be used to calculate the user's creation
    /// date.
    pub user_id: UserId,
    /// The associated user instance.
    pub user: Option<Arc<RwLock<User>>>,
    pub(crate) _nonexhaustive: (),
}

impl<'de> Deserialize<'de> for Presence {
    fn deserialize<D>(deserializer: D) -> StdResult<Presence, D::Error>
        where
            D: Deserializer<'de>
    {
        #[derive(Deserialize)]
        #[serde(field_identifier)]
        #[serde(rename_all = "snake_case")]
        enum UserField {
            Id,
            Avatar,
            Bot,
            Discriminator,
            Username,
        }

        struct UserVisitor;

        impl<'de> DeserializeSeed<'de> for UserVisitor {
            type Value = (UserId, Option<Arc<RwLock<User>>>);

            fn deserialize<D>(self, deserializer: D) -> StdResult<Self::Value, D::Error>
                where
                    D: Deserializer<'de>
            {
                deserializer.deserialize_any(self)
            }
        }

        impl<'de> Visitor<'de> for UserVisitor {
            type Value = (UserId, Option<Arc<RwLock<User>>>);

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt.write_str("struct User")
            }

            fn visit_map<M>(self, mut map: M) -> StdResult<Self::Value, M::Error>
                where
                    M: MapAccess<'de>,
            {
                let mut id = None;
                let mut avatar = None;
                let mut bot = None;
                let mut discriminator = None;
                let mut name = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        UserField::Id => {
                            if id.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            id = Some(map.next_value()?);
                        }
                        UserField::Avatar => {
                            if avatar.is_some() {
                                return Err(de::Error::duplicate_field("avatar"));
                            }
                            avatar = Some(map.next_value()?);
                        }
                        UserField::Bot => {
                            if bot.is_some() {
                                return Err(de::Error::duplicate_field("bot"));
                            }
                            bot = Some(map.next_value()?);
                        }
                        UserField::Discriminator => {
                            if discriminator.is_some() {
                                return Err(de::Error::duplicate_field("discriminator"));
                            }
                            discriminator = Some(map.next_value()?);
                        }
                        UserField::Username => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("username"));
                            }
                            name = Some(map.next_value()?);
                        }
                    }
                }
                let id = id.ok_or_else(|| de::Error::missing_field("id"))?;
                let bot = bot.unwrap_or_default();

                if let (Some(d), Some(n)) = (discriminator, name) {
                    Ok((id, Some(Arc::new(RwLock::new(User {
                        id,
                        avatar,
                        bot,
                        discriminator: d,
                        name: n,
                        _nonexhaustive: (),
                    })))))
                } else {
                    Ok((id, None))
                }
            }
        }

        #[derive(Deserialize)]
        #[serde(field_identifier)]
        #[serde(rename_all = "snake_case")]
        enum Field {
            User,
            Game,
            LastModified,
            Nick,
            Status,
        }

        struct PresenceVisitor;

        impl<'de> Visitor<'de> for PresenceVisitor {
            type Value = Presence;

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt.write_str("struct Presence")
            }

            fn visit_map<M>(self, mut map: M) -> StdResult<Self::Value, M::Error>
                where
                    M: MapAccess<'de>,
            {
                let mut user_id = None;
                let mut user = None;
                let mut activity = None;
                let mut last_modified = None;
                let mut nick = None;
                let mut status = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::User => {
                            if user.is_some() {
                                return Err(de::Error::duplicate_field("user"));
                            }
                            let (uid, u) = map.next_value_seed(UserVisitor)?;
                            user_id = Some(uid);
                            user = u;
                        }
                        Field::Game => {
                            if activity.is_some() {
                                return Err(de::Error::duplicate_field("game"));
                            }
                            activity = Some(map.next_value()?);
                        }
                        Field::LastModified => {
                            if last_modified.is_some() {
                                return Err(de::Error::duplicate_field("last_modified"));
                            }
                            last_modified = Some(map.next_value()?);
                        }
                        Field::Nick => {
                            if nick.is_some() {
                                return Err(de::Error::duplicate_field("nick"));
                            }
                            nick = Some(map.next_value()?);
                        }
                        Field::Status => {
                            if status.is_some() {
                                return Err(de::Error::duplicate_field("status"));
                            }
                            status = Some(map.next_value()?);
                        }
                    }
                }
                let status = status.ok_or_else(|| de::Error::missing_field("status"))?;
                let user_id = user_id.ok_or_else(|| de::Error::missing_field("user_id"))?;

                Ok(Presence { activity, last_modified, nick, status, user_id, user, _nonexhaustive: () })
            }
        }

        const FIELDS: &[&str] = &["user", "game", "last_modified", "nick", "status"];
        deserializer.deserialize_struct("Presence", FIELDS, PresenceVisitor)
    }
}

impl Serialize for Presence {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where
            S: Serializer
    {
        let mut state = serializer.serialize_struct("Presence", 5)?;
        state.serialize_field("game", &self.activity)?;
        state.serialize_field("last_modified", &self.last_modified)?;
        state.serialize_field("nick", &self.nick)?;
        state.serialize_field("status", &self.status)?;

        if let Some(ref user) = self.user {
            state.serialize_field("user", &*user.read())?;
        } else {
            let mut map = HashMap::new();
            map.insert("id", &self.user_id);

            state.serialize_field("user", &map)?;
        }

        state.end()
    }
}

/// An initial set of information given after IDENTIFYing to the gateway.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Ready {
    pub guilds: Vec<GuildStatus>,
    #[serde(default, serialize_with = "serialize_presences", deserialize_with = "deserialize_presences")]
    pub presences: HashMap<UserId, Presence>,
    #[serde(default, serialize_with = "serialize_private_channels", deserialize_with = "deserialize_private_channels")]
    pub private_channels: HashMap<ChannelId, Channel>,
    pub session_id: String,
    pub shard: Option<[u64; 2]>,
    #[serde(default, rename = "_trace")]
    pub trace: Vec<String>,
    pub user: CurrentUser,
    #[serde(rename = "v")]
    pub version: u64,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

/// Information describing how many gateway sessions you can initiate within a
/// ratelimit period.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SessionStartLimit {
    /// The number of sessions that you can still initiate within the current
    /// ratelimit period.
    pub remaining: u64,
    /// The number of milliseconds until the ratelimit period resets.
    pub reset_after: u64,
    /// The total number of session starts within the ratelimit period allowed.
    pub total: u64,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}
/// Timestamps of when a user started and/or is ending their activity.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityTimestamps {
    pub end: Option<u64>,
    pub start: Option<u64>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}
