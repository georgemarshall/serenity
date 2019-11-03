//! All the events this library handles.

use chrono::{DateTime, FixedOffset};
use serde::de::{self, Deserialize, DeserializeSeed, Error as DeError, MapAccess, SeqAccess};
use serde::ser::{
    Serialize,
    SerializeSeq,
    Serializer
};
use std::{
    collections::HashMap,
    fmt,
    marker::PhantomData,
};
use super::utils::deserialize_emojis;
use super::prelude::*;
use crate::constants::{OpCode, VoiceOpCode};
use crate::internal::{
    de::{Content, ContentDeserializer, OptionallyTaggedContentVisitor, size_hint},
    prelude::*,
};

#[cfg(feature = "cache")]
use crate::cache::{Cache, CacheUpdate};
#[cfg(feature = "cache")]
use crate::internal::RwLockExt;
#[cfg(feature = "cache")]
use std::collections::hash_map::Entry;
#[cfg(feature = "cache")]
use std::mem;

/// Event data for the channel creation event.
///
/// This is fired when:
///
/// - A [`Channel`] is created in a [`Guild`]
/// - A [`PrivateChannel`] is created
/// - The current user is added to a [`Group`]
///
/// [`Channel`]: ../channel/enum.Channel.html
/// [`Group`]: ../channel/struct.Group.html
/// [`Guild`]: ../guild/struct.Guild.html
/// [`PrivateChannel`]: ../channel/struct.PrivateChannel.html
#[derive(Clone, Debug)]
pub struct ChannelCreateEvent {
    /// The channel that was created.
    pub channel: Channel,
    pub(crate) _nonexhaustive: (),
}

impl<'de> Deserialize<'de> for ChannelCreateEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            channel: Channel::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for ChannelCreateEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        Channel::serialize(&self.channel, serializer)
    }
}

#[cfg(feature = "cache")]
impl CacheUpdate for ChannelCreateEvent {
    type Output = Channel;

    fn update(&mut self, cache: &mut Cache) -> Option<Self::Output> {
        match self.channel {
            Channel::Group(ref group) => {
                let group = Arc::clone(group);

                let channel_id = group.with_mut(|writer| {
                    for (recipient_id, recipient) in &mut writer.recipients {
                        cache.update_user_entry(&recipient.read());

                        *recipient = Arc::clone(&cache.users[recipient_id]);
                    }

                    writer.channel_id
                });

                let ch = cache.groups.insert(channel_id, group);

                ch.map(Channel::Group)
            },
            Channel::Guild(ref channel) => {
                let (guild_id, channel_id) = channel.with(|channel| (channel.guild_id, channel.id));

                cache.channels.insert(channel_id, Arc::clone(channel));

                cache
                    .guilds
                    .get_mut(&guild_id)
                    .and_then(|guild| {
                        guild
                            .with_mut(|guild| guild.channels.insert(channel_id, Arc::clone(channel)))
                    })
                    .map(Channel::Guild)
            },
            Channel::Private(ref channel) => {
                if let Some(channel) = cache.private_channels.get(&channel.with(|c| c.id)) {
                    return Some(Channel::Private(Arc::clone(&(*channel))));
                }

                let channel = Arc::clone(channel);

                let id = channel.with_mut(|writer| {
                    let user_id = writer.recipient.with_mut(|user| {
                        cache.update_user_entry(user);

                        user.id
                    });

                    writer.recipient = Arc::clone(&cache.users[&user_id]);
                    writer.id
                });

                let ch = cache.private_channels.insert(id, Arc::clone(&channel));
                ch.map(Channel::Private)
            },
            Channel::Category(ref category) => cache
                .categories
                .insert(category.read().id, Arc::clone(category))
                .map(Channel::Category),
            Channel::__Nonexhaustive => unreachable!(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChannelDeleteEvent {
    pub channel: Channel,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for ChannelDeleteEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        match self.channel {
            Channel::Guild(ref channel) => {
                let (guild_id, channel_id) = channel.with(|channel| (channel.guild_id, channel.id));

                cache.channels.remove(&channel_id);

                cache
                    .guilds
                    .get_mut(&guild_id)
                    .and_then(|guild| guild.with_mut(|g| g.channels.remove(&channel_id)));
            },
            Channel::Category(ref category) => {
                let channel_id = category.with(|cat| cat.id);

                cache.categories.remove(&channel_id);
            },
            Channel::Private(ref channel) => {
                let id = {
                    channel.read().id
                };

                cache.private_channels.remove(&id);
            },

            // We ignore these because the delete event does not fire for these.
            Channel::Group(_) |
            Channel::__Nonexhaustive => unreachable!(),
        };

        // Remove the cached messages for the channel.
        cache.messages.remove(&self.channel.id());

        None
    }
}

impl<'de> Deserialize<'de> for ChannelDeleteEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            channel: Channel::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for ChannelDeleteEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        Channel::serialize(&self.channel, serializer)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChannelPinsUpdateEvent {
    pub channel_id: ChannelId,
    pub last_pin_timestamp: Option<DateTime<FixedOffset>>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for ChannelPinsUpdateEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        if let Some(channel) = cache.channels.get(&self.channel_id) {
            channel.with_mut(|c| {
                c.last_pin_timestamp = self.last_pin_timestamp;
            });

            return None;
        }

        if let Some(channel) = cache.private_channels.get_mut(&self.channel_id) {
            channel.with_mut(|c| {
                c.last_pin_timestamp = self.last_pin_timestamp;
            });

            return None;
        }

        if let Some(group) = cache.groups.get_mut(&self.channel_id) {
            group.with_mut(|c| {
                c.last_pin_timestamp = self.last_pin_timestamp;
            });

            return None;
        }

        None
    }
}


#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChannelRecipientAddEvent {
    pub channel_id: ChannelId,
    pub user: User,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for ChannelRecipientAddEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        cache.update_user_entry(&self.user);
        let user = Arc::clone(&cache.users[&self.user.id]);

        if let Some(group) = cache.groups.get_mut(&self.channel_id) {
            group.write().recipients.insert(self.user.id, user);
        }

        None
    }
}


#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChannelRecipientRemoveEvent {
    pub channel_id: ChannelId,
    pub user: User,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for ChannelRecipientRemoveEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        cache.groups.get_mut(&self.channel_id).map(|group| {
            group.with_mut(|g| g.recipients.remove(&self.user.id))
        });

        None
    }
}

#[derive(Clone, Debug)]
pub struct ChannelUpdateEvent {
    pub channel: Channel,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for ChannelUpdateEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        match self.channel {
            Channel::Group(ref group) => {
                let (ch_id, no_recipients) =
                    group.with(|g| (g.channel_id, g.recipients.is_empty()));

                match cache.groups.entry(ch_id) {
                    Entry::Vacant(e) => {
                        e.insert(Arc::clone(group));
                    },
                    Entry::Occupied(mut e) => {
                        let mut dest = e.get_mut().write();

                        if no_recipients {
                            let recipients = mem::replace(&mut dest.recipients, HashMap::new());

                            dest.clone_from(&group.read());

                            dest.recipients = recipients;
                        } else {
                            dest.clone_from(&group.read());
                        }
                    },
                }
            },
            Channel::Guild(ref channel) => {
                let (guild_id, channel_id) = channel.with(|channel| (channel.guild_id, channel.id));

                cache.channels.insert(channel_id, Arc::clone(channel));

                if let Some(guild) = cache.guilds.get_mut(&guild_id) {
                    guild
                        .with_mut(|g| g.channels.insert(channel_id, Arc::clone(channel)));
                }
            },
            Channel::Private(ref channel) => {
                if let Some(private) = cache.private_channels.get_mut(&channel.read().id) {
                    private.clone_from(channel);
                }
            },
            Channel::Category(ref category) => {
                if let Some(c) = cache
                    .categories
                    .get_mut(&category.read().id)
                    { c.clone_from(category) }
            },
            Channel::__Nonexhaustive => unreachable!(),
        }

        None
    }
}

impl<'de> Deserialize<'de> for ChannelUpdateEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            channel: Channel::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for ChannelUpdateEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        Channel::serialize(&self.channel, serializer)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildBanAddEvent {
    pub guild_id: GuildId,
    pub user: User,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildBanRemoveEvent {
    pub guild_id: GuildId,
    pub user: User,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug)]
pub struct GuildCreateEvent {
    pub guild: Guild,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildCreateEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        cache.unavailable_guilds.remove(&self.guild.id);

        let mut guild = self.guild.clone();

        for (user_id, member) in &mut guild.members {
            cache.update_user_entry(&member.user.read());
            let user = Arc::clone(&cache.users[user_id]);

            member.user = Arc::clone(&user);
        }

        cache.channels.extend(guild.channels.clone());
        cache
            .guilds
            .insert(self.guild.id, Arc::new(RwLock::new(guild)));

        None
    }
}

impl<'de> Deserialize<'de> for GuildCreateEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            guild: Guild::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for GuildCreateEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        Guild::serialize(&self.guild, serializer)
    }
}

#[derive(Clone, Debug)]
pub struct GuildDeleteEvent {
    pub guild: PartialGuild,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildDeleteEvent {
    type Output = Arc<RwLock<Guild>>;

    fn update(&mut self, cache: &mut Cache) -> Option<Self::Output> {
        // Remove channel entries for the guild if the guild is found.
        cache.guilds.remove(&self.guild.id).map(|guild| {
            for channel_id in guild.write().channels.keys() {
                // Remove the channel from the cache.
                cache.channels.remove(channel_id);

                // Remove the channel's cached messages.
                cache.messages.remove(channel_id);
            }

            guild
        })
    }
}

impl<'de> Deserialize<'de> for GuildDeleteEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            guild: PartialGuild::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for GuildDeleteEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        PartialGuild::serialize(&self.guild, serializer)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildEmojisUpdateEvent {
    #[serde(serialize_with = "serialize_emojis", deserialize_with = "deserialize_emojis")] pub emojis: HashMap<EmojiId, Emoji>,
    pub guild_id: GuildId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildEmojisUpdateEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        if let Some(guild) = cache.guilds.get_mut(&self.guild_id) {
            guild.with_mut(|g| {
                g.emojis.clone_from(&self.emojis)
            });
        }

        None
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildIntegrationsUpdateEvent {
    pub guild_id: GuildId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug)]
pub struct GuildMemberAddEvent {
    pub guild_id: GuildId,
    pub member: Member,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildMemberAddEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        let user_id = self.member.user.with(|u| u.id);
        cache.update_user_entry(&self.member.user.read());

        // Always safe due to being inserted above.
        self.member.user = Arc::clone(&cache.users[&user_id]);

        if let Some(guild) = cache.guilds.get_mut(&self.guild_id) {
            guild.with_mut(|guild| {
                guild.member_count += 1;
                guild.members.insert(user_id, self.member.clone());
            });
        }

        None
    }
}

impl<'de> Deserialize<'de> for GuildMemberAddEvent {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
        where
            D: Deserializer<'de>
    {
        let member = Member::deserialize(deserializer)?;

        Ok(GuildMemberAddEvent {
            // Duplicate `guild_id` since it is already contained by `Member`
            guild_id: member.guild_id,
            member,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for GuildMemberAddEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where
            S: Serializer
    {
        // Skip `guild_id` since it is already contained by `Member`
        self.member.serialize(serializer)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildMemberRemoveEvent {
    pub guild_id: GuildId,
    pub user: User,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildMemberRemoveEvent {
    type Output = Member;

    fn update(&mut self, cache: &mut Cache) -> Option<Self::Output> {
        cache.guilds.get_mut(&self.guild_id).and_then(|guild| {
            guild.with_mut(|guild| {
                guild.member_count -= 1;
                guild.members.remove(&self.user.id)
            })
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildMemberUpdateEvent {
    pub guild_id: GuildId,
    pub nick: Option<String>,
    pub roles: Vec<RoleId>,
    pub user: User,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildMemberUpdateEvent {
    type Output = Member;

    fn update(&mut self, cache: &mut Cache) -> Option<Self::Output> {
        cache.update_user_entry(&self.user);

        if let Some(guild) = cache.guilds.get_mut(&self.guild_id) {
            let mut guild = guild.write();

            let mut found = false;

            let item = if let Some(member) = guild.members.get_mut(&self.user.id) {
                let item = Some(member.clone());

                member.nick.clone_from(&self.nick);
                member.roles.clone_from(&self.roles);
                member.user.write().clone_from(&self.user);

                found = true;

                item
            } else {
                None
            };

            if !found {
                guild.members.insert(
                    self.user.id,
                    Member {
                        deaf: false,
                        guild_id: self.guild_id,
                        joined_at: None,
                        mute: false,
                        nick: self.nick.clone(),
                        roles: self.roles.clone(),
                        user: Arc::new(RwLock::new(self.user.clone())),
                        _nonexhaustive: (),
                    },
                );
            }

            item
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GuildMembersChunkEvent {
    pub guild_id: GuildId,
    pub members: HashMap<UserId, Member>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildMembersChunkEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        for member in self.members.values() {
            cache.update_user_entry(&member.user.read());
        }

        if let Some(guild) = cache.guilds.get_mut(&self.guild_id) {
            guild.with_mut(|g| g.members.extend(self.members.clone()))
        }

        None
    }
}

impl<'de> Deserialize<'de> for GuildMembersChunkEvent {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
        where
            D: Deserializer<'de>
    {
        #[derive(Deserialize)]
        #[serde(field_identifier)]
        #[serde(rename_all = "snake_case")]
        enum MemberField {
            Deaf,
            GuildId,
            JoinedAt,
            Mute,
            Nick,
            Roles,
            User,
        }

        struct MemberVisitor {
            guild_id: GuildId,
        }

        impl MemberVisitor {
            pub fn new(guild_id: GuildId) -> Self {
                MemberVisitor { guild_id }
            }
        }

        impl<'de> DeserializeSeed<'de> for MemberVisitor {
            type Value = Member;

            fn deserialize<D>(self, deserializer: D) -> StdResult<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
            {
                deserializer.deserialize_any(self)
            }
        }

        impl<'de> Visitor<'de> for MemberVisitor {
            type Value = Member;

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt.write_str("struct Member")
            }

            fn visit_map<M>(self, mut map: M) -> StdResult<Self::Value, M::Error>
                where
                    M: MapAccess<'de>,
            {
                let mut deaf = None;
                let mut guild_id = None;
                let mut joined_at = None;
                let mut mute = None;
                let mut nick = None;
                let mut roles = None;
                let mut user = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        MemberField::Deaf => {
                            if deaf.is_some() {
                                return Err(de::Error::duplicate_field("deaf"));
                            }
                            deaf = Some(map.next_value()?);
                        }
                        MemberField::GuildId => {
                            if guild_id.is_some() {
                                return Err(de::Error::duplicate_field("guild_id"));
                            }
                            guild_id = Some(map.next_value()?);
                        }
                        MemberField::JoinedAt => {
                            if joined_at.is_some() {
                                return Err(de::Error::duplicate_field("joined_at"));
                            }
                            joined_at = Some(map.next_value()?);
                        }
                        MemberField::Mute => {
                            if mute.is_some() {
                                return Err(de::Error::duplicate_field("mute"));
                            }
                            mute = Some(map.next_value()?);
                        }
                        MemberField::Nick => {
                            if nick.is_some() {
                                return Err(de::Error::duplicate_field("nick"));
                            }
                            nick = Some(map.next_value()?);
                        }
                        MemberField::Roles => {
                            if roles.is_some() {
                                return Err(de::Error::duplicate_field("roles"));
                            }
                            roles = Some(map.next_value()?);
                        }
                        MemberField::User => {
                            if user.is_some() {
                                return Err(de::Error::duplicate_field("user"));
                            }
                            user = Some(Arc::new(RwLock::new(map.next_value()?)));
                        }
                    }
                }
                let deaf = deaf.ok_or_else(|| de::Error::missing_field("deaf"))?;
                let guild_id = guild_id.unwrap_or(self.guild_id);
                let joined_at = joined_at.ok_or_else(|| de::Error::missing_field("joined_at"))?;
                let mute = mute.ok_or_else(|| de::Error::missing_field("mute"))?;
                let roles = roles.ok_or_else(|| de::Error::missing_field("roles"))?;
                let user = user.ok_or_else(|| de::Error::missing_field("user"))?;

                Ok(Member { deaf, guild_id, joined_at, mute, nick, roles, user, _nonexhaustive: () })
            }
        }

        struct MemberSeqVisitor {
            guild_id: GuildId,
        }

        impl MemberSeqVisitor {
            pub fn new(guild_id: GuildId) -> Self {
                MemberSeqVisitor { guild_id }
            }
        }

        impl<'de> Visitor<'de> for MemberSeqVisitor {
            type Value = HashMap<UserId, Member>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a sequence struct Member")
            }

            fn visit_seq<S>(self, mut seq: S) -> StdResult<Self::Value, S::Error>
                where
                    S: SeqAccess<'de>,
            {
                let mut member_map = HashMap::with_capacity(size_hint::cautious(seq.size_hint()));
                while let Some(member) = seq.next_element_seed(MemberVisitor::new(self.guild_id))? {
                    let member_id = member.user.read().id;
                    if member_map.contains_key(&member_id) {
                        return Err(de::Error::custom(format_args!("duplicate member `{}`", member_id)));
                    }
                    member_map.insert(member_id, member);
                }
                Ok(member_map)
            }
        }

        #[derive(Deserialize)]
        #[serde(field_identifier)]
        #[serde(rename_all = "snake_case")]
        enum Field {
            GuildId,
            Members,
            NotFound,
        }

        struct GuildMembersChunkEventVisitor;

        impl<'de> Visitor<'de> for GuildMembersChunkEventVisitor {
            type Value = GuildMembersChunkEvent;

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt.write_str("struct GuildMembersChunkEvent")
            }

            fn visit_map<M>(self, mut map: M) -> StdResult<Self::Value, M::Error>
                where
                    M: MapAccess<'de>,
            {
                let mut guild_id = None;
                let mut members = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::GuildId => {
                            if guild_id.is_some() {
                                return Err(de::Error::duplicate_field("guild_id"));
                            }
                            guild_id = Some(map.next_value()?);
                        }
                        Field::Members => {
                            if members.is_some() {
                                return Err(de::Error::duplicate_field("members"));
                            }
                            members = Some(map.next_value()?);
                        }
                        Field::NotFound => (),
                    }
                }
                let guild_id = guild_id.ok_or_else(|| de::Error::missing_field("guild_id"))?;
                let members = members.ok_or_else(|| de::Error::missing_field("members"))?;

                let deserializer = ContentDeserializer::new(members);

                Ok(GuildMembersChunkEvent {
                    guild_id,
                    members: deserializer.deserialize_seq(MemberSeqVisitor::new(guild_id))?,
                    _nonexhaustive: (),
                })
            }
        }

        const FIELDS: &[&str] = &["guild_id", "members", "not_found"];
        deserializer.deserialize_struct("GuildMembersChunkEvent", FIELDS, GuildMembersChunkEventVisitor)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildRoleCreateEvent {
    pub guild_id: GuildId,
    pub role: Role,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildRoleCreateEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        cache.guilds.get_mut(&self.guild_id).map(|guild| {
            guild
                .write()
                .roles
                .insert(self.role.id, self.role.clone())
        });

        None
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildRoleDeleteEvent {
    pub guild_id: GuildId,
    pub role_id: RoleId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildRoleDeleteEvent {
    type Output = Role;

    fn update(&mut self, cache: &mut Cache) -> Option<Self::Output> {
        cache
            .guilds
            .get_mut(&self.guild_id)
            .and_then(|guild| guild.with_mut(|g| g.roles.remove(&self.role_id)))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildRoleUpdateEvent {
    pub guild_id: GuildId,
    pub role: Role,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildRoleUpdateEvent {
    type Output = Role;

    fn update(&mut self, cache: &mut Cache) -> Option<Self::Output> {
        cache.guilds.get_mut(&self.guild_id).and_then(|guild| {
            guild.with_mut(|g| {
                g.roles
                    .get_mut(&self.role.id)
                    .map(|role| mem::replace(role, self.role.clone()))
            })
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildUnavailableEvent {
    #[serde(rename = "id")] pub guild_id: GuildId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildUnavailableEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        cache.unavailable_guilds.insert(self.guild_id);
        cache.guilds.remove(&self.guild_id);

        None
    }
}

#[derive(Clone, Debug)]
pub struct GuildUpdateEvent {
    pub guild: PartialGuild,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for GuildUpdateEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        if let Some(guild) = cache.guilds.get_mut(&self.guild.id) {
            let mut guild = guild.write();

            guild.afk_timeout = self.guild.afk_timeout;
            guild.afk_channel_id.clone_from(&self.guild.afk_channel_id);
            guild.icon.clone_from(&self.guild.icon);
            guild.name.clone_from(&self.guild.name);
            guild.owner_id.clone_from(&self.guild.owner_id);
            guild.region.clone_from(&self.guild.region);
            guild.roles.clone_from(&self.guild.roles);
            guild.verification_level = self.guild.verification_level;
        }

        None
    }
}

impl<'de> Deserialize<'de> for GuildUpdateEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            guild: PartialGuild::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for GuildUpdateEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        PartialGuild::serialize(&self.guild, serializer)
    }
}

#[derive(Clone, Debug)]
pub struct MessageCreateEvent {
    pub message: Message,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for MessageCreateEvent {
    /// The oldest message, if the channel's message cache was already full.
    type Output = Message;

    fn update(&mut self, cache: &mut Cache) -> Option<Self::Output> {
        let max = cache.settings().max_messages;

        if max == 0 {
            return None;
        }

        let messages = cache.messages
            .entry(self.message.channel_id)
            .or_insert_with(Default::default);
        let queue = cache.message_queue
            .entry(self.message.channel_id)
            .or_insert_with(Default::default);

        let mut removed_msg = None;

        if messages.len() == max {
            if let Some(id) = queue.pop_front() {
                removed_msg = messages.remove(&id);
            }
        }

        queue.push_back(self.message.id);
        messages.insert(self.message.id, self.message.clone());

        removed_msg
    }
}

impl<'de> Deserialize<'de> for MessageCreateEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            message: Message::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for MessageCreateEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        Message::serialize(&self.message, serializer)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageDeleteBulkEvent {
    pub channel_id: ChannelId,
    pub ids: Vec<MessageId>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct MessageDeleteEvent {
    pub channel_id: ChannelId,
    #[serde(rename = "id")] pub message_id: MessageId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageUpdateEvent {
    pub id: MessageId,
    pub channel_id: ChannelId,
    pub kind: Option<MessageType>,
    pub content: Option<String>,
    pub nonce: Option<String>,
    pub tts: Option<bool>,
    pub pinned: Option<bool>,
    pub timestamp: Option<DateTime<FixedOffset>>,
    pub edited_timestamp: Option<DateTime<FixedOffset>>,
    pub author: Option<User>,
    pub mention_everyone: Option<bool>,
    pub mentions: Option<Vec<User>>,
    pub mention_roles: Option<Vec<RoleId>>,
    pub attachments: Option<Vec<Attachment>>,
    pub embeds: Option<Vec<Value>>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for MessageUpdateEvent {
    type Output = Message;

    fn update(&mut self, cache: &mut Cache) -> Option<Self::Output> {
        if let Some(messages) = cache.messages.get_mut(&self.channel_id) {

            if let Some(message) = messages.get_mut(&self.id) {
                let item = message.clone();

                if let Some(attachments) = self.attachments.clone() {
                    message.attachments = attachments;
                }

                if let Some(content) = self.content.clone() {
                    message.content = content;
                }

                if let Some(edited_timestamp) = self.edited_timestamp {
                    message.edited_timestamp = Some(edited_timestamp);
                }

                if let Some(mentions) = self.mentions.clone() {
                    message.mentions = mentions;
                }

                if let Some(mention_everyone) = self.mention_everyone {
                    message.mention_everyone = mention_everyone;
                }

                if let Some(mention_roles) = self.mention_roles.clone() {
                    message.mention_roles = mention_roles;
                }

                if let Some(pinned) = self.pinned {
                    message.pinned = pinned;
                }

                return Some(item);
            }
        }

        None
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PresenceUpdateEvent {
    pub guild_id: Option<GuildId>,
    #[serde(flatten)]
    pub presence: Presence,
    pub roles: Option<Vec<RoleId>>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for PresenceUpdateEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        let user_id = self.presence.user_id;

        if let Some(user) = self.presence.user.as_mut() {
            cache.update_user_entry(&user.read());
            *user = Arc::clone(&cache.users[&user_id]);
        }

        if let Some(guild_id) = self.guild_id {
            if let Some(guild) = cache.guilds.get_mut(&guild_id) {
                let mut guild = guild.write();

                // If the member went offline, remove them from the presence list.
                if self.presence.status == OnlineStatus::Offline {
                    guild.presences.remove(&self.presence.user_id);
                } else {
                    guild
                        .presences
                        .insert(self.presence.user_id, self.presence.clone());
                }

                // Create a partial member instance out of the presence update
                // data. This includes everything but `deaf`, `mute`, and
                // `joined_at`.
                if !guild.members.contains_key(&self.presence.user_id) {
                    if let Some(user) = self.presence.user.as_ref() {
                        let roles = self.roles.clone().unwrap_or_default();

                        guild.members.insert(self.presence.user_id, Member {
                            deaf: false,
                            guild_id,
                            joined_at: None,
                            mute: false,
                            nick: self.presence.nick.clone(),
                            user: Arc::clone(&user),
                            roles,
                            _nonexhaustive: (),
                        });
                    }
                }
            }
        } else if self.presence.status == OnlineStatus::Offline {
            cache.presences.remove(&self.presence.user_id);
        } else {
            cache
                .presences
                .insert(self.presence.user_id, self.presence.clone());
        }

        None
    }
}

#[derive(Clone, Debug)]
pub struct PresencesReplaceEvent {
    pub presences: Vec<Presence>,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for PresencesReplaceEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        cache.presences.extend({
            let mut p: HashMap<UserId, Presence> = HashMap::default();

            for presence in &self.presences {
                p.insert(presence.user_id, presence.clone());
            }

            p
        });

        None
    }
}

impl<'de> Deserialize<'de> for PresencesReplaceEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        let presences: Vec<Presence> = Deserialize::deserialize(deserializer)?;

        Ok(Self {
            presences,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for PresencesReplaceEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(self.presences.len()))?;

        for value in &self.presences {
            seq.serialize_element(value)?;
        }

        seq.end()
    }
}

#[derive(Clone, Debug)]
pub struct ReactionAddEvent {
    pub reaction: Reaction,
    pub(crate) _nonexhaustive: (),
}

impl<'de> Deserialize<'de> for ReactionAddEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            reaction: Reaction::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for ReactionAddEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        Reaction::serialize(&self.reaction, serializer)
    }
}

#[derive(Clone, Debug)]
pub struct ReactionRemoveEvent {
    pub reaction: Reaction,
    pub(crate) _nonexhaustive: (),
}

impl<'de> Deserialize<'de> for ReactionRemoveEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            reaction: Reaction::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for ReactionRemoveEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        Reaction::serialize(&self.reaction, serializer)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct ReactionRemoveAllEvent {
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

/// The "Ready" event, containing initial ready cache
#[derive(Clone, Debug)]
pub struct ReadyEvent {
    pub ready: Ready,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for ReadyEvent {
    type Output = ();

    fn update(&mut self, cache: &mut Cache) -> Option<()> {
        let mut ready = self.ready.clone();

        for guild in ready.guilds {
            match guild {
                GuildStatus::Offline(unavailable) => {
                    cache.guilds.remove(&unavailable.id);
                    cache.unavailable_guilds.insert(unavailable.id);
                },
                GuildStatus::OnlineGuild(guild) => {
                    cache.unavailable_guilds.remove(&guild.id);
                    cache.guilds.insert(guild.id, Arc::new(RwLock::new(guild)));
                },
                GuildStatus::OnlinePartialGuild(_) => {},
                GuildStatus::__Nonexhaustive => unreachable!(),
            }
        }

        // `ready.private_channels` will always be empty, and possibly be removed in the future.
        // So don't handle it at all.

        for (user_id, presence) in &mut ready.presences {
            if let Some(ref user) = presence.user {
                cache.update_user_entry(&user.read());
            }

            presence.user = cache.users.get(user_id).cloned();
        }

        cache.presences.extend(ready.presences);
        cache.shard_count = ready.shard.map_or(1, |s| s[1]);
        cache.user = ready.user;

        None
    }
}

impl<'de> Deserialize<'de> for ReadyEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            ready: Ready::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for ReadyEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        Ready::serialize(&self.ready, serializer)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResumedEvent {
    #[serde(rename = "_trace")] pub trace: Vec<Option<String>>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TypingStartEvent {
    pub channel_id: ChannelId,
    pub timestamp: u64,
    pub user_id: UserId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UnknownEvent {
    pub kind: String,
    pub value: Value,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug)]
pub struct UserUpdateEvent {
    pub current_user: CurrentUser,
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for UserUpdateEvent {
    type Output = CurrentUser;

    fn update(&mut self, cache: &mut Cache) -> Option<Self::Output> {
        Some(mem::replace(&mut cache.user, self.current_user.clone()))
    }
}

impl<'de> Deserialize<'de> for UserUpdateEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> StdResult<Self, D::Error> {
        Ok(Self {
            current_user: CurrentUser::deserialize(deserializer)?,
            _nonexhaustive: (),
        })
    }
}

impl Serialize for UserUpdateEvent {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
        where S: Serializer {
        CurrentUser::serialize(&self.current_user, serializer)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceServerUpdateEvent {
    pub channel_id: Option<ChannelId>,
    pub endpoint: Option<String>,
    pub guild_id: Option<GuildId>,
    pub token: String,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceStateUpdateEvent {
    pub guild_id: Option<GuildId>,
    #[serde(flatten)]
    pub voice_state: VoiceState,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[cfg(feature = "cache")]
impl CacheUpdate for VoiceStateUpdateEvent {
    type Output = VoiceState;

    fn update(&mut self, cache: &mut Cache) -> Option<VoiceState> {
        if let Some(guild_id) = self.guild_id {
            if let Some(guild) = cache.guilds.get_mut(&guild_id) {
                let mut guild = guild.write();

                if self.voice_state.channel_id.is_some() {
                    // Update or add to the voice state list
                    guild
                        .voice_states
                        .insert(self.voice_state.user_id, self.voice_state.clone())
                } else {
                    // Remove the user from the voice state list
                    guild.voice_states.remove(&self.voice_state.user_id)
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebhookUpdateEvent {
    pub channel_id: ChannelId,
    pub guild_id: GuildId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    // _trace: Vec<String>,
    pub heartbeat_interval: u64,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum GatewayEvent {
    Dispatch(u64, Event),
    Heartbeat(u64),
    Reconnect,
    /// Whether the session can be resumed.
    InvalidSession(bool),
    Hello(u64),
    HeartbeatAck,
    #[doc(hidden)]
    __Nonexhaustive,
}

impl<'de> Deserialize<'de> for GatewayEvent {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
        where
            D: Deserializer<'de>
    {
        pub struct GatewayPayload<'a> {
            pub opcode: OpCode,
            pub data: Content<'a>,
            pub sequence: Option<u64>,
            pub event_type: Option<EventType>,
        }

        // The code bellow replicates the functionality of the generated code from
        // serde. However the generated code has issues with lifetime inference and must
        // be implemented manually until fixed.
        //
        // #[derive(Deserialize)]
        // #[serde(deny_unknown_fields)]
        // pub struct GatewayPayload<'a> {
        //     #[serde(rename = "op")]
        //     pub opcode: OpCode,
        //     #[serde(borrow)]
        //     #[serde(rename = "d")]
        //     pub data: Content<'a>,
        //     #[serde(rename = "s")]
        //     pub sequence: Option<u64>,
        //     #[serde(rename = "t")]
        //     pub event_type: Option<EventType>,
        // }
        impl<'de: 'a, 'a> Deserialize<'de> for GatewayPayload<'a> {
            fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
                where
                    D: Deserializer<'de>
            {
                #[derive(Deserialize)]
                #[serde(field_identifier)]
                enum Field {
                    #[serde(rename = "op")]
                    OpCode,
                    #[serde(rename = "d")]
                    Data,
                    #[serde(rename = "s")]
                    Sequence,
                    #[serde(rename = "t")]
                    Type,
                }

                struct GatewayPayloadVisitor<'de: 'a, 'a> {
                    marker: PhantomData<GatewayPayload<'a>>,
                    lifetime: PhantomData<&'de ()>,
                }

                impl<'de: 'a, 'a> Visitor<'de> for GatewayPayloadVisitor<'de, 'a> {
                    type Value = GatewayPayload<'a>;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("struct GatewayPayload")
                    }

                    fn visit_map<V>(self, mut map: V) -> StdResult<Self::Value, V::Error>
                        where
                            V: MapAccess<'de>,
                    {
                        let mut opcode = None;
                        let mut data = None;
                        let mut sequence = None;
                        let mut event_type = None;
                        while let Some(key) = map.next_key()? {
                            match key {
                                Field::OpCode => {
                                    if opcode.is_some() {
                                        return Err(de::Error::duplicate_field("op"));
                                    }
                                    opcode = Some(map.next_value()?);
                                }
                                Field::Data => {
                                    if data.is_some() {
                                        return Err(de::Error::duplicate_field("d"));
                                    }
                                    data = Some(map.next_value()?);
                                }
                                Field::Sequence => {
                                    if sequence.is_some() {
                                        return Err(de::Error::duplicate_field("s"));
                                    }
                                    sequence = Some(map.next_value()?);
                                }
                                Field::Type => {
                                    if event_type.is_some() {
                                        return Err(de::Error::duplicate_field("t"));
                                    }
                                    event_type = Some(map.next_value()?);
                                }
                            }
                        }
                        let opcode = opcode.ok_or_else(|| de::Error::missing_field("op"))?;
                        let data = data.ok_or_else(|| de::Error::missing_field("d"))?;
                        let sequence = sequence.ok_or_else(|| de::Error::missing_field("s"))?;
                        let event_type = event_type.ok_or_else(|| de::Error::missing_field("t"))?;

                        Ok(GatewayPayload { opcode, data, sequence, event_type })
                    }
                }

                const FIELDS: &[&str] = &["op", "d", "s", "t"];
                deserializer.deserialize_struct("GatewayPayload", FIELDS, GatewayPayloadVisitor { marker: PhantomData::<GatewayPayload<'a>>, lifetime: PhantomData })
            }
        }

        struct GatewayEventVisitor {
            opcode: OpCode,
            sequence: Option<u64>,
            event_type: Option<EventType>,
        }

        impl GatewayEventVisitor {
            pub fn new(opcode: OpCode, sequence: Option<u64>, event_type: Option<EventType>) -> Self {
                GatewayEventVisitor { opcode, sequence, event_type }
            }
        }

        impl<'de> DeserializeSeed<'de> for GatewayEventVisitor {
            type Value = GatewayEvent;

            fn deserialize<D>(self, deserializer: D) -> StdResult<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
            {
                Ok(match self.opcode {
                    OpCode::Dispatch => {
                        let s = self.sequence.ok_or_else(|| de::Error::invalid_value(de::Unexpected::Option, &"sequence value"))?;
                        let kind = self.event_type.ok_or_else(|| de::Error::invalid_value(de::Unexpected::Option, &"evnent type"))?;
                        let seed = EventSeed::new(kind);
                        let x = seed.deserialize(deserializer)?;

                        GatewayEvent::Dispatch(s, x)
                    }
                    OpCode::Heartbeat => {
                        let s = self.sequence.ok_or_else(|| de::Error::invalid_value(de::Unexpected::Option, &"sequence value"))?;

                        GatewayEvent::Heartbeat(s)
                    }
                    OpCode::Reconnect => GatewayEvent::Reconnect,
                    OpCode::InvalidSession => {
                        let resumable = bool::deserialize(deserializer)?;

                        GatewayEvent::InvalidSession(resumable)
                    }
                    OpCode::Hello => {
                        let hello = Hello::deserialize(deserializer)?;

                        GatewayEvent::Hello(hello.heartbeat_interval)
                    }
                    OpCode::HeartbeatAck => GatewayEvent::HeartbeatAck,
                    _ => return Err(de::Error::unknown_variant(&format!("{:?}", self.opcode), &["Dispatch", "Heartbeat", "Reconnect", "Reconnect", "InvalidSession", "Hello", "HeartbeatAck"])),
                })
            }
        }

        let GatewayPayload { opcode, data, sequence, event_type } = GatewayPayload::deserialize(deserializer)?;

        let visitor = GatewayEventVisitor::new(opcode, sequence, event_type);
        visitor.deserialize(ContentDeserializer::new(data))
    }
}

/// Event received over a websocket connection
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Event {
    /// A [`Channel`] was created.
    ///
    /// Fires the [`EventHandler::channel_create`] event.
    ///
    /// [`Channel`]: ../channel/enum.Channel.html
    /// [`EventHandler::channel_create`]: ../../client/trait.EventHandler.html#method.channel_create
    ChannelCreate(ChannelCreateEvent),
    /// A [`Channel`] has been deleted.
    ///
    /// Fires the [`EventHandler::channel_delete`] event.
    ///
    /// [`Channel`]: ../channel/enum.Channel.html
    /// [`EventHandler::channel_delete`]: ../../client/trait.EventHandler.html#method.channel_delete
    ChannelDelete(ChannelDeleteEvent),
    /// The pins for a [`Channel`] have been updated.
    ///
    /// Fires the [`EventHandler::channel_pins_update`] event.
    ///
    /// [`Channel`]: ../enum.Channel.html
    /// [`EventHandler::channel_pins_update`]:
    /// ../../client/trait.EventHandler.html#method.channel_pins_update
    ChannelPinsUpdate(ChannelPinsUpdateEvent),
    /// A [`User`] has been added to a [`Group`].
    ///
    /// Fires the [`EventHandler::channel_recipient_addition`] event.
    ///
    /// [`EventHandler::channel_recipient_addition`]: ../../client/trait.EventHandler.html#method.channel_recipient_addition
    /// [`User`]: ../struct.User.html
    ChannelRecipientAdd(ChannelRecipientAddEvent),
    /// A [`User`] has been removed from a [`Group`].
    ///
    /// Fires the [`EventHandler::channel_recipient_removal`] event.
    ///
    /// [`EventHandler::channel_recipient_removal`]: ../../client/trait.EventHandler.html#method.channel_recipient_removal
    /// [`User`]: ../struct.User.html
    ChannelRecipientRemove(ChannelRecipientRemoveEvent),
    /// A [`Channel`] has been updated.
    ///
    /// Fires the [`EventHandler::channel_update`] event.
    ///
    /// [`EventHandler::channel_update`]: ../../client/trait.EventHandler.html#method.channel_update
    /// [`User`]: ../struct.User.html
    ChannelUpdate(ChannelUpdateEvent),
    GuildBanAdd(GuildBanAddEvent),
    GuildBanRemove(GuildBanRemoveEvent),
    GuildCreate(GuildCreateEvent),
    GuildDelete(GuildDeleteEvent),
    GuildEmojisUpdate(GuildEmojisUpdateEvent),
    GuildIntegrationsUpdate(GuildIntegrationsUpdateEvent),
    GuildMemberAdd(GuildMemberAddEvent),
    GuildMemberRemove(GuildMemberRemoveEvent),
    /// A member's roles have changed
    GuildMemberUpdate(GuildMemberUpdateEvent),
    GuildMembersChunk(GuildMembersChunkEvent),
    GuildRoleCreate(GuildRoleCreateEvent),
    GuildRoleDelete(GuildRoleDeleteEvent),
    GuildRoleUpdate(GuildRoleUpdateEvent),
    /// When a guild is unavailable, such as due to a Discord server outage.
    GuildUnavailable(GuildUnavailableEvent),
    GuildUpdate(GuildUpdateEvent),
    MessageCreate(MessageCreateEvent),
    MessageDelete(MessageDeleteEvent),
    MessageDeleteBulk(MessageDeleteBulkEvent),
    /// A message has been edited, either by the user or the system
    MessageUpdate(MessageUpdateEvent),
    /// A member's presence state (or username or avatar) has changed
    PresenceUpdate(PresenceUpdateEvent),
    /// The precense list of the user's friends should be replaced entirely
    PresencesReplace(PresencesReplaceEvent),
    /// A reaction was added to a message.
    ///
    /// Fires the [`EventHandler::reaction_add`] event handler.
    ///
    /// [`EventHandler::reaction_add`]: ../../client/trait.EventHandler.html#method.reaction_add
    ReactionAdd(ReactionAddEvent),
    /// A reaction was removed to a message.
    ///
    /// Fires the [`EventHandler::reaction_remove`] event handler.
    ///
    /// [`EventHandler::reaction_remove`]:
    /// ../../client/trait.EventHandler.html#method.reaction_remove
    ReactionRemove(ReactionRemoveEvent),
    /// A request was issued to remove all [`Reaction`]s from a [`Message`].
    ///
    /// Fires the [`EventHandler::reaction_remove_all`] event handler.
    ///
    /// [`Message`]: struct.Message.html
    /// [`Reaction`]: struct.Reaction.html
    /// [`EventHandler::reaction_remove_all`]: ../../client/trait.EventHandler.html#method.reaction_remove_all
    ReactionRemoveAll(ReactionRemoveAllEvent),
    /// The first event in a connection, containing the initial ready cache.
    ///
    /// May also be received at a later time in the event of a reconnect.
    Ready(ReadyEvent),
    /// The connection has successfully resumed after a disconnect.
    Resumed(ResumedEvent),
    /// A user is typing; considered to last 5 seconds
    TypingStart(TypingStartEvent),
    /// Update to the logged-in user's information
    UserUpdate(UserUpdateEvent),
    /// A member's voice state has changed
    VoiceStateUpdate(VoiceStateUpdateEvent),
    /// Voice server information is available
    VoiceServerUpdate(VoiceServerUpdateEvent),
    /// A webhook for a [channel][`GuildChannel`] was updated in a [`Guild`].
    ///
    /// [`Guild`]: struct.Guild.html
    /// [`GuildChannel`]: struct.GuildChannel.html
    WebhookUpdate(WebhookUpdateEvent),
    /// An event type not covered by the above
    Unknown(UnknownEvent),
    #[doc(hidden)]
    __Nonexhaustive,
}

struct EventSeed {
    event_type: EventType,
}

impl EventSeed {
    fn new(event_type: EventType) -> Self {
        EventSeed { event_type }
    }
}

impl<'de> DeserializeSeed<'de> for EventSeed {
    type Value = Event;

    /// Deserializes a `serde_json::Value` into an `Event`.
    ///
    /// The given `EventType` is used to determine what event to deserialize into.
    /// For example, an [`EventType::ChannelCreate`] will cause the given value to
    /// attempt to be deserialized into a [`ChannelCreateEvent`].
    ///
    /// Special handling is done in regards to [`EventType::GuildCreate`] and
    /// [`EventType::GuildDelete`]: they check for an `"unavailable"` key and, if
    /// present and containing a value of `true`, will cause a
    /// [`GuildUnavailableEvent`] to be returned. Otherwise, all other event types
    /// correlate to the deserialization of their appropriate event.
    ///
    /// [`EventType::ChannelCreate`]: enum.EventType.html#variant.ChannelCreate
    /// [`EventType::GuildCreate`]: enum.EventType.html#variant.GuildCreate
    /// [`EventType::GuildDelete`]: enum.EventType.html#variant.GuildDelete
    /// [`ChannelCreateEvent`]: struct.ChannelCreateEvent.html
    /// [`GuildUnavailableEvent`]: struct.GuildUnavailableEvent.html
    fn deserialize<D>(self, deserializer: D) -> StdResult<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
    {
        Ok(match self.event_type {
            EventType::ChannelCreate => Event::ChannelCreate(ChannelCreateEvent::deserialize(deserializer)?),
            EventType::ChannelDelete => Event::ChannelDelete(ChannelDeleteEvent::deserialize(deserializer)?),
            EventType::ChannelPinsUpdate => {
                Event::ChannelPinsUpdate(ChannelPinsUpdateEvent::deserialize(deserializer)?)
            }
            EventType::ChannelRecipientAdd => {
                Event::ChannelRecipientAdd(ChannelRecipientAddEvent::deserialize(deserializer)?)
            }
            EventType::ChannelRecipientRemove => {
                Event::ChannelRecipientRemove(ChannelRecipientRemoveEvent::deserialize(deserializer)?)
            }
            EventType::ChannelUpdate => Event::ChannelUpdate(ChannelUpdateEvent::deserialize(deserializer)?),
            EventType::GuildBanAdd => Event::GuildBanAdd(GuildBanAddEvent::deserialize(deserializer)?),
            EventType::GuildBanRemove => Event::GuildBanRemove(GuildBanRemoveEvent::deserialize(deserializer)?),
            EventType::GuildCreate | EventType::GuildUnavailable => {
                // GuildUnavailable isn't actually received from the gateway, so it
                // can be lumped in with GuildCreate's arm.
                let visitor = OptionallyTaggedContentVisitor::new("unavailable");
                let optionally_tagged = deserializer.deserialize_map(visitor)?;

                let de = ContentDeserializer::new(optionally_tagged.content);
                if optionally_tagged.tag.unwrap_or(false) {
                    Event::GuildUnavailable(GuildUnavailableEvent::deserialize(de)?)
                } else {
                    Event::GuildCreate(GuildCreateEvent::deserialize(de)?)
                }
            }
            EventType::GuildDelete => {
                let visitor = OptionallyTaggedContentVisitor::new("unavailable");
                let optionally_tagged = deserializer.deserialize_map(visitor)?;

                let de = ContentDeserializer::new(optionally_tagged.content);
                if optionally_tagged.tag.unwrap_or(false) {
                    Event::GuildUnavailable(GuildUnavailableEvent::deserialize(de)?)
                } else {
                    Event::GuildDelete(GuildDeleteEvent::deserialize(de)?)
                }
            }
            EventType::GuildEmojisUpdate => {
                Event::GuildEmojisUpdate(GuildEmojisUpdateEvent::deserialize(deserializer)?)
            }
            EventType::GuildIntegrationsUpdate => {
                Event::GuildIntegrationsUpdate(GuildIntegrationsUpdateEvent::deserialize(deserializer)?)
            }
            EventType::GuildMemberAdd => Event::GuildMemberAdd(GuildMemberAddEvent::deserialize(deserializer)?),
            EventType::GuildMemberRemove => {
                Event::GuildMemberRemove(GuildMemberRemoveEvent::deserialize(deserializer)?)
            }
            EventType::GuildMemberUpdate => {
                Event::GuildMemberUpdate(GuildMemberUpdateEvent::deserialize(deserializer)?)
            }
            EventType::GuildMembersChunk => {
                Event::GuildMembersChunk(GuildMembersChunkEvent::deserialize(deserializer)?)
            }
            EventType::GuildRoleCreate => {
                Event::GuildRoleCreate(GuildRoleCreateEvent::deserialize(deserializer)?)
            }
            EventType::GuildRoleDelete => {
                Event::GuildRoleDelete(GuildRoleDeleteEvent::deserialize(deserializer)?)
            }
            EventType::GuildRoleUpdate => {
                Event::GuildRoleUpdate(GuildRoleUpdateEvent::deserialize(deserializer)?)
            }
            EventType::GuildUpdate => Event::GuildUpdate(GuildUpdateEvent::deserialize(deserializer)?),
            EventType::MessageCreate => Event::MessageCreate(MessageCreateEvent::deserialize(deserializer)?),
            EventType::MessageDelete => Event::MessageDelete(MessageDeleteEvent::deserialize(deserializer)?),
            EventType::MessageDeleteBulk => {
                Event::MessageDeleteBulk(MessageDeleteBulkEvent::deserialize(deserializer)?)
            }
            EventType::MessageReactionAdd => {
                Event::ReactionAdd(ReactionAddEvent::deserialize(deserializer)?)
            }
            EventType::MessageReactionRemove => {
                Event::ReactionRemove(ReactionRemoveEvent::deserialize(deserializer)?)
            }
            EventType::MessageReactionRemoveAll => {
                Event::ReactionRemoveAll(ReactionRemoveAllEvent::deserialize(deserializer)?)
            }
            EventType::MessageUpdate => Event::MessageUpdate(MessageUpdateEvent::deserialize(deserializer)?),
            EventType::PresenceUpdate => Event::PresenceUpdate(PresenceUpdateEvent::deserialize(deserializer)?),
            EventType::PresencesReplace => {
                Event::PresencesReplace(PresencesReplaceEvent::deserialize(deserializer)?)
            }
            EventType::Ready => Event::Ready(ReadyEvent::deserialize(deserializer)?),
            EventType::Resumed => Event::Resumed(ResumedEvent::deserialize(deserializer)?),
            EventType::TypingStart => Event::TypingStart(TypingStartEvent::deserialize(deserializer)?),
            EventType::UserUpdate => Event::UserUpdate(UserUpdateEvent::deserialize(deserializer)?),
            EventType::VoiceServerUpdate => {
                Event::VoiceServerUpdate(VoiceServerUpdateEvent::deserialize(deserializer)?)
            }
            EventType::VoiceStateUpdate => {
                Event::VoiceStateUpdate(VoiceStateUpdateEvent::deserialize(deserializer)?)
            }
            EventType::WebhooksUpdate => Event::WebhookUpdate(WebhookUpdateEvent::deserialize(deserializer)?),
            EventType::Other(kind) => Event::Unknown(UnknownEvent {
                kind: kind.to_owned(),
                value: Value::deserialize(deserializer)?,
                _nonexhaustive: (),
            }),
            EventType::__Nonexhaustive => unreachable!(),
        })
    }
}

/// The type of event dispatch received from the gateway.
///
/// This is useful for deciding how to deserialize a received payload.
///
/// A Deserialization implementation is provided for deserializing raw event
/// dispatch type strings to this enum, e.g. deserializing `"CHANNEL_CREATE"` to
/// [`EventType::ChannelCreate`].
///
/// [`EventType::ChannelCreate`]: enum.EventType.html#variant.ChannelCreate
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum EventType {
    /// Indicator that a channel create payload was received.
    ///
    /// This maps to [`ChannelCreateEvent`].
    ///
    /// [`ChannelCreateEvent`]: struct.ChannelCreateEvent.html
    ChannelCreate,
    /// Indicator that a channel delete payload was received.
    ///
    /// This maps to [`ChannelDeleteEvent`].
    ///
    /// [`ChannelDeleteEvent`]: struct.ChannelDeleteEvent.html
    ChannelDelete,
    /// Indicator that a channel pins update payload was received.
    ///
    /// This maps to [`ChannelPinsUpdateEvent`].
    ///
    /// [`ChannelPinsUpdateEvent`]: struct.ChannelPinsUpdateEvent.html
    ChannelPinsUpdate,
    /// Indicator that a channel recipient addition payload was received.
    ///
    /// This maps to [`ChannelRecipientAddEvent`].
    ///
    /// [`ChannelRecipientAddEvent`]: struct.ChannelRecipientAddEvent.html
    ChannelRecipientAdd,
    /// Indicator that a channel recipient removal payload was received.
    ///
    /// This maps to [`ChannelRecipientRemoveEvent`].
    ///
    /// [`ChannelRecipientRemoveEvent`]: struct.ChannelRecipientRemoveEvent.html
    ChannelRecipientRemove,
    /// Indicator that a channel update payload was received.
    ///
    /// This maps to [`ChannelUpdateEvent`].
    ///
    /// [`ChannelUpdateEvent`]: struct.ChannelUpdateEvent.html
    ChannelUpdate,
    /// Indicator that a guild ban addition payload was received.
    ///
    /// This maps to [`GuildBanAddEvent`].
    ///
    /// [`GuildBanAddEvent`]: struct.GuildBanAddEvent.html
    GuildBanAdd,
    /// Indicator that a guild ban removal payload was received.
    ///
    /// This maps to [`GuildBanRemoveEvent`].
    ///
    /// [`GuildBanRemoveEvent`]: struct.GuildBanRemoveEvent.html
    GuildBanRemove,
    /// Indicator that a guild create payload was received.
    ///
    /// This maps to [`GuildCreateEvent`].
    ///
    /// [`GuildCreateEvent`]: struct.GuildCreateEvent.html
    GuildCreate,
    /// Indicator that a guild delete payload was received.
    ///
    /// This maps to [`GuildDeleteEvent`].
    ///
    /// [`GuildDeleteEvent`]: struct.GuildDeleteEvent.html
    GuildDelete,
    /// Indicator that a guild emojis update payload was received.
    ///
    /// This maps to [`GuildEmojisUpdateEvent`].
    ///
    /// [`GuildEmojisUpdateEvent`]: struct.GuildEmojisUpdateEvent.html
    GuildEmojisUpdate,
    /// Indicator that a guild integrations update payload was received.
    ///
    /// This maps to [`GuildIntegrationsUpdateEvent`].
    ///
    /// [`GuildIntegrationsUpdateEvent`]: struct.GuildIntegrationsUpdateEvent.html
    GuildIntegrationsUpdate,
    /// Indicator that a guild member add payload was received.
    ///
    /// This maps to [`GuildMemberAddEvent`].
    ///
    /// [`GuildMemberAddEvent`]: struct.GuildMemberAddEvent.html
    GuildMemberAdd,
    /// Indicator that a guild member remove payload was received.
    ///
    /// This maps to [`GuildMemberRemoveEvent`].
    ///
    /// [`GuildMemberRemoveEvent`]: struct.GuildMemberRemoveEvent.html
    GuildMemberRemove,
    /// Indicator that a guild member update payload was received.
    ///
    /// This maps to [`GuildMemberUpdateEvent`].
    ///
    /// [`GuildMemberUpdateEvent`]: struct.GuildMemberUpdateEvent.html
    GuildMemberUpdate,
    /// Indicator that a guild members chunk payload was received.
    ///
    /// This maps to [`GuildMembersChunkEvent`].
    ///
    /// [`GuildMembersChunkEvent`]: struct.GuildMembersChunkEvent.html
    GuildMembersChunk,
    /// Indicator that a guild role create payload was received.
    ///
    /// This maps to [`GuildRoleCreateEvent`].
    ///
    /// [`GuildRoleCreateEvent`]: struct.GuildRoleCreateEvent.html
    GuildRoleCreate,
    /// Indicator that a guild role delete payload was received.
    ///
    /// This maps to [`GuildRoleDeleteEvent`].
    ///
    /// [`GuildRoleDeleteEvent`]: struct.GuildRoleDeleteEvent.html
    GuildRoleDelete,
    /// Indicator that a guild role update payload was received.
    ///
    /// This maps to [`GuildRoleUpdateEvent`].
    ///
    /// [`GuildRoleUpdateEvent`]: struct.GuildRoleUpdateEvent.html
    GuildRoleUpdate,
    /// Indicator that a guild unavailable payload was received.
    ///
    /// This maps to [`GuildUnavailableEvent`].
    ///
    /// [`GuildUnavailableEvent`]: struct.GuildUnavailableEvent.html
    GuildUnavailable,
    /// Indicator that a guild update payload was received.
    ///
    /// This maps to [`GuildUpdateEvent`].
    ///
    /// [`GuildUpdateEvent`]: struct.GuildUpdateEvent.html
    GuildUpdate,
    /// Indicator that a message create payload was received.
    ///
    /// This maps to [`MessageCreateEvent`].
    ///
    /// [`MessageCreateEvent`]: struct.MessageCreateEvent.html
    MessageCreate,
    /// Indicator that a message delete payload was received.
    ///
    /// This maps to [`MessageDeleteEvent`].
    ///
    /// [`MessageDeleteEvent`]: struct.MessageDeleteEvent.html
    MessageDelete,
    /// Indicator that a message delete bulk payload was received.
    ///
    /// This maps to [`MessageDeleteBulkEvent`].
    ///
    /// [`MessageDeleteBulkEvent`]: struct.MessageDeleteBulkEvent.html
    MessageDeleteBulk,
    /// Indicator that a message update payload was received.
    ///
    /// This maps to [`MessageUpdateEvent`].
    ///
    /// [`MessageUpdateEvent`]: struct.MessageUpdateEvent.html
    MessageUpdate,
    /// Indicator that a presence update payload was received.
    ///
    /// This maps to [`PresenceUpdateEvent`].
    ///
    /// [`PresenceUpdateEvent`]: struct.PresenceUpdateEvent.html
    PresenceUpdate,
    /// Indicator that a presences replace payload was received.
    ///
    /// This maps to [`PresencesReplaceEvent`].
    ///
    /// [`PresencesReplaceEvent`]: struct.PresencesReplaceEvent.html
    PresencesReplace,
    /// Indicator that a reaction add payload was received.
    ///
    /// This maps to [`ReactionAddEvent`].
    ///
    /// [`ReactionAddEvent`]: struct.ReactionAddEvent.html
    MessageReactionAdd,
    /// Indicator that a reaction remove payload was received.
    ///
    /// This maps to [`ReactionRemoveEvent`].
    ///
    /// [`ReactionRemoveEvent`]: struct.ResumedEvent.html
    MessageReactionRemove,
    /// Indicator that a reaction remove all payload was received.
    ///
    /// This maps to [`ReactionRemoveAllEvent`].
    ///
    /// [`ReactionRemoveAllEvent`]: struct.ReactionRemoveAllEvent.html
    MessageReactionRemoveAll,
    /// Indicator that a ready payload was received.
    ///
    /// This maps to [`ReadyEvent`].
    ///
    /// [`ReadyEvent`]: struct.ReadyEvent.html
    Ready,
    /// Indicator that a resumed payload was received.
    ///
    /// This maps to [`ResumedEvent`].
    ///
    /// [`ResumedEvent`]: struct.ResumedEvent.html
    Resumed,
    /// Indicator that a typing start payload was received.
    ///
    /// This maps to [`TypingStartEvent`].
    ///
    /// [`TypingStartEvent`]: struct.TypingStartEvent.html
    TypingStart,
    /// Indicator that a user update payload was received.
    ///
    /// This maps to [`UserUpdateEvent`].
    ///
    /// [`UserUpdateEvent`]: struct.UserUpdateEvent.html
    UserUpdate,
    /// Indicator that a voice state payload was received.
    ///
    /// This maps to [`VoiceStateUpdateEvent`].
    ///
    /// [`VoiceStateUpdateEvent`]: struct.VoiceStateUpdateEvent.html
    VoiceStateUpdate,
    /// Indicator that a voice server update payload was received.
    ///
    /// This maps to [`VoiceServerUpdateEvent`].
    ///
    /// [`VoiceServerUpdateEvent`]: struct.VoiceServerUpdateEvent.html
    VoiceServerUpdate,
    /// Indicator that a webhook update payload was received.
    ///
    /// This maps to [`WebhookUpdateEvent`].
    ///
    /// [`WebhookUpdateEvent`]: struct.WebhookUpdateEvent.html
    WebhooksUpdate,
    /// An unknown event was received over the gateway.
    ///
    /// This should be logged so that support for it can be added in the
    /// library.
    Other(String),
    #[doc(hidden)]
    __Nonexhaustive,
}

impl<'de> Deserialize<'de> for EventType {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
        where D: Deserializer<'de> {
        struct EventTypeVisitor;

        impl<'de> Visitor<'de> for EventTypeVisitor {
            type Value = EventType;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("event type str")
            }

            fn visit_str<E>(self, v: &str) -> StdResult<Self::Value, E>
                where E: DeError {
                Ok(match v {
                    "CHANNEL_CREATE" => EventType::ChannelCreate,
                    "CHANNEL_DELETE" => EventType::ChannelDelete,
                    "CHANNEL_PINS_UPDATE" => EventType::ChannelPinsUpdate,
                    "CHANNEL_RECIPIENT_ADD" => EventType::ChannelRecipientAdd,
                    "CHANNEL_RECIPIENT_REMOVE" => EventType::ChannelRecipientRemove,
                    "CHANNEL_UPDATE" => EventType::ChannelUpdate,
                    "GUILD_BAN_ADD" => EventType::GuildBanAdd,
                    "GUILD_BAN_REMOVE" => EventType::GuildBanRemove,
                    "GUILD_CREATE" => EventType::GuildCreate,
                    "GUILD_DELETE" => EventType::GuildDelete,
                    "GUILD_EMOJIS_UPDATE" => EventType::GuildEmojisUpdate,
                    "GUILD_INTEGRATIONS_UPDATE" => EventType::GuildIntegrationsUpdate,
                    "GUILD_MEMBER_ADD" => EventType::GuildMemberAdd,
                    "GUILD_MEMBER_REMOVE" => EventType::GuildMemberRemove,
                    "GUILD_MEMBER_UPDATE" => EventType::GuildMemberUpdate,
                    "GUILD_MEMBERS_CHUNK" => EventType::GuildMembersChunk,
                    "GUILD_ROLE_CREATE" => EventType::GuildRoleCreate,
                    "GUILD_ROLE_DELETE" => EventType::GuildRoleDelete,
                    "GUILD_ROLE_UPDATE" => EventType::GuildRoleUpdate,
                    "GUILD_UPDATE" => EventType::GuildUpdate,
                    "MESSAGE_CREATE" => EventType::MessageCreate,
                    "MESSAGE_DELETE" => EventType::MessageDelete,
                    "MESSAGE_DELETE_BULK" => EventType::MessageDeleteBulk,
                    "MESSAGE_REACTION_ADD" => EventType::MessageReactionAdd,
                    "MESSAGE_REACTION_REMOVE" => EventType::MessageReactionRemove,
                    "MESSAGE_REACTION_REMOVE_ALL" => EventType::MessageReactionRemoveAll,
                    "MESSAGE_UPDATE" => EventType::MessageUpdate,
                    "PRESENCE_UPDATE" => EventType::PresenceUpdate,
                    "PRESENCES_REPLACE" => EventType::PresencesReplace,
                    "READY" => EventType::Ready,
                    "RESUMED" => EventType::Resumed,
                    "TYPING_START" => EventType::TypingStart,
                    "USER_UPDATE" => EventType::UserUpdate,
                    "VOICE_SERVER_UPDATE" => EventType::VoiceServerUpdate,
                    "VOICE_STATE_UPDATE" => EventType::VoiceStateUpdate,
                    "WEBHOOKS_UPDATE" => EventType::WebhooksUpdate,
                    other => EventType::Other(other.to_owned()),
                })
            }
        }

        deserializer.deserialize_str(EventTypeVisitor)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct VoiceHeartbeat {
    pub nonce: u64,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct VoiceHeartbeatAck {
    pub nonce: u64,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceReady {
    pub heartbeat_interval: u64,
    pub modes: Vec<String>,
    pub ip: String, 
    pub port: u16,
    pub ssrc: u32,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceHello {
    pub heartbeat_interval: u64,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceSessionDescription {
    pub mode: String,
    pub secret_key: Vec<u8>,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct VoiceSpeaking {
    pub speaking: bool,
    pub ssrc: u32,
    pub user_id: UserId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceResume {
    pub server_id: String,
    pub session_id: String,
    pub token: String,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct VoiceClientConnect {
    pub audio_ssrc: u32,
    pub user_id: UserId,
    pub video_ssrc: u32,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct VoiceClientDisconnect {
    pub user_id: UserId,
    #[serde(skip)]
    pub(crate) _nonexhaustive: (),
}

/// A representation of data received for [`voice`] events.
///
/// [`voice`]: ../../voice/index.html
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum VoiceEvent {
    /// Server's response to the client's Identify operation.
    /// Contains session-specific information, e.g.
    /// [`ssrc`] and supported encryption modes.
    ///
    /// [`ssrc`]: struct.VoiceReady.html#structfield.ssrc
    Ready(VoiceReady),
    /// A voice event describing the current session.
    SessionDescription(VoiceSessionDescription),
    /// A voice event denoting that someone is speaking.
    Speaking(VoiceSpeaking),
    /// Acknowledgement from the server for a prior voice heartbeat.
    HeartbeatAck(VoiceHeartbeatAck),
    /// A "hello" was received with initial voice data, such as the
    /// true [`heartbeat_interval`].
    ///
    /// [`heartbeat_interval`]: struct.VoiceHello.html#structfield.heartbeat_interval
    Hello(VoiceHello),
    /// Message received if a Resume request was successful.
    Resumed,
    /// Status update in the current channel, indicating that a user has
    /// connected.
    ClientConnect(VoiceClientConnect),
    /// Status update in the current channel, indicating that a user has
    /// disconnected.
    ClientDisconnect(VoiceClientDisconnect),
    /// An unknown voice event not registered.
    Unknown(VoiceOpCode, Value),
    #[doc(hidden)]
    __Nonexhaustive,
}

impl<'de> Deserialize<'de> for VoiceEvent {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
        where
            D: Deserializer<'de>
    {
        pub struct GatewayPayload<'a> {
            pub opcode: VoiceOpCode,
            pub data: Content<'a>,
        }

        // The code bellow replicates the functionality of the generated code from
        // serde. However the generated code has issues with lifetime inference and must
        // be implemented manually until fixed.
        //
        // #[derive(Deserialize)]
        // #[serde(deny_unknown_fields)]
        // pub struct GatewayPayload<'a> {
        //     #[serde(rename = "op")]
        //     pub opcode: VoiceOpCode,
        //     #[serde(borrow)]
        //     #[serde(rename = "d")]
        //     pub data: Content<'a>,
        // }
        impl<'de: 'a, 'a> Deserialize<'de> for GatewayPayload<'a> {
            fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
                where
                    D: Deserializer<'de>
            {
                #[derive(Deserialize)]
                #[serde(field_identifier)]
                enum Field {
                    #[serde(rename = "op")]
                    OpCode,
                    #[serde(rename = "d")]
                    Data,
                }

                struct GatewayPayloadVisitor<'de: 'a, 'a> {
                    marker: PhantomData<GatewayPayload<'a>>,
                    lifetime: PhantomData<&'de ()>,
                }

                impl<'de: 'a, 'a> Visitor<'de> for GatewayPayloadVisitor<'de, 'a> {
                    type Value = GatewayPayload<'a>;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("struct GatewayPayload")
                    }

                    fn visit_map<V>(self, mut map: V) -> StdResult<Self::Value, V::Error>
                        where
                            V: MapAccess<'de>,
                    {
                        let mut opcode = None;
                        let mut data = None;
                        while let Some(key) = map.next_key()? {
                            match key {
                                Field::OpCode => {
                                    if opcode.is_some() {
                                        return Err(de::Error::duplicate_field("op"));
                                    }
                                    opcode = Some(map.next_value()?);
                                }
                                Field::Data => {
                                    if data.is_some() {
                                        return Err(de::Error::duplicate_field("d"));
                                    }
                                    data = Some(map.next_value()?);
                                }
                            }
                        }
                        let opcode = opcode.ok_or_else(|| de::Error::missing_field("op"))?;
                        let data = data.ok_or_else(|| de::Error::missing_field("d"))?;

                        Ok(GatewayPayload { opcode, data })
                    }
                }

                const FIELDS: &[&str] = &["op", "d", ];
                deserializer.deserialize_struct("GatewayPayload", FIELDS, GatewayPayloadVisitor { marker: PhantomData::<GatewayPayload<'a>>, lifetime: PhantomData })
            }
        }

        struct VoiceEventVisitor {
            opcode: VoiceOpCode,
        }

        impl VoiceEventVisitor {
            pub fn new(opcode: VoiceOpCode) -> Self {
                VoiceEventVisitor { opcode }
            }
        }

        impl<'de> DeserializeSeed<'de> for VoiceEventVisitor {
            type Value = VoiceEvent;

            fn deserialize<D>(self, deserializer: D) -> StdResult<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
            {
                Ok(match self.opcode {
                    VoiceOpCode::HeartbeatAck => {
                        VoiceEvent::HeartbeatAck(VoiceHeartbeatAck::deserialize(deserializer)?)
                    }
                    VoiceOpCode::Ready => {
                        VoiceEvent::Ready(VoiceReady::deserialize(deserializer)?)
                    }
                    VoiceOpCode::Hello => {
                        VoiceEvent::Hello(VoiceHello::deserialize(deserializer)?)
                    }
                    VoiceOpCode::SessionDescription => {
                        VoiceEvent::SessionDescription(VoiceSessionDescription::deserialize(deserializer)?)
                    }
                    VoiceOpCode::Speaking => {
                        VoiceEvent::Speaking(VoiceSpeaking::deserialize(deserializer)?)
                    }
                    VoiceOpCode::Resumed => VoiceEvent::Resumed,
                    VoiceOpCode::ClientConnect => {
                        VoiceEvent::ClientConnect(VoiceClientConnect::deserialize(deserializer)?)
                    }
                    VoiceOpCode::ClientDisconnect => {
                        VoiceEvent::ClientDisconnect(VoiceClientDisconnect::deserialize(deserializer)?)
                    }
                    other => VoiceEvent::Unknown(other, Value::deserialize(deserializer)?),
                })
            }
        }

        let GatewayPayload { opcode, data } = GatewayPayload::deserialize(deserializer)?;

        let visitor = VoiceEventVisitor::new(opcode);
        visitor.deserialize(ContentDeserializer::new(data))
    }
}
