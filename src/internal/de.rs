use serde::{
    de::{self, Deserialize, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess, Visitor},
};
use std::{
    fmt,
    marker::PhantomData,
};
pub use serde::private::de::{Content, ContentDeserializer, size_hint};

struct ContentVisitor<'de> {
    value: PhantomData<Content<'de>>,
}

impl<'de> ContentVisitor<'de> {
    fn new() -> Self {
        ContentVisitor { value: PhantomData }
    }
}

impl<'de> Visitor<'de> for ContentVisitor<'de> {
    type Value = Content<'de>;

    fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str("any value")
    }

    fn visit_bool<F>(self, value: bool) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::Bool(value))
    }

    fn visit_i8<F>(self, value: i8) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::I8(value))
    }

    fn visit_i16<F>(self, value: i16) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::I16(value))
    }

    fn visit_i32<F>(self, value: i32) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::I32(value))
    }

    fn visit_i64<F>(self, value: i64) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::I64(value))
    }

    fn visit_u8<F>(self, value: u8) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::U8(value))
    }

    fn visit_u16<F>(self, value: u16) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::U16(value))
    }

    fn visit_u32<F>(self, value: u32) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::U32(value))
    }

    fn visit_u64<F>(self, value: u64) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::U64(value))
    }

    fn visit_f32<F>(self, value: f32) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::F32(value))
    }

    fn visit_f64<F>(self, value: f64) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::F64(value))
    }

    fn visit_char<F>(self, value: char) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::Char(value))
    }

    fn visit_str<F>(self, value: &str) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::String(value.into()))
    }

    fn visit_borrowed_str<F>(self, value: &'de str) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::Str(value))
    }

    fn visit_string<F>(self, value: String) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::String(value))
    }

    fn visit_bytes<F>(self, value: &[u8]) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::ByteBuf(value.into()))
    }

    fn visit_borrowed_bytes<F>(self, value: &'de [u8]) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::Bytes(value))
    }

    fn visit_byte_buf<F>(self, value: Vec<u8>) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::ByteBuf(value))
    }

    fn visit_none<F>(self) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|v| Content::Some(Box::new(v)))
    }

    fn visit_unit<F>(self) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        Ok(Content::Unit)
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|v| Content::Newtype(Box::new(v)))
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
        where
            V: SeqAccess<'de>,
    {
        let mut vec = Vec::with_capacity(size_hint::cautious(visitor.size_hint()));
        while let Some(e) = visitor.next_element()? {
            vec.push(e);
        }
        Ok(Content::Seq(vec))
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
        where
            V: MapAccess<'de>,
    {
        let mut vec = Vec::with_capacity(size_hint::cautious(visitor.size_hint()));
        while let Some(kv) = visitor.next_entry()? {
            vec.push(kv);
        }
        Ok(Content::Map(vec))
    }

    fn visit_enum<V>(self, _visitor: V) -> Result<Self::Value, V::Error>
        where
            V: EnumAccess<'de>,
    {
        Err(de::Error::custom(
            "untagged and internally tagged enums do not support enum input",
        ))
    }
}

pub enum TagOrContent<'de> {
    Tag,
    Content(Content<'de>),
}

struct TagOrContentVisitor<'de> {
    name: &'static str,
    value: PhantomData<TagOrContent<'de>>,
}

impl<'de> TagOrContentVisitor<'de> {
    fn new(name: &'static str) -> Self {
        TagOrContentVisitor {
            name,
            value: PhantomData,
        }
    }
}

impl<'de> DeserializeSeed<'de> for TagOrContentVisitor<'de> {
    type Value = TagOrContent<'de>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
    {
        // Internally tagged enums are only supported in self-describing
        // formats.
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for TagOrContentVisitor<'de> {
    type Value = TagOrContent<'de>;

    fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "a type tag `{}` or any other value", self.name)
    }

    fn visit_bool<F>(self, value: bool) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_bool(value)
            .map(TagOrContent::Content)
    }

    fn visit_i8<F>(self, value: i8) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_i8(value)
            .map(TagOrContent::Content)
    }

    fn visit_i16<F>(self, value: i16) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_i16(value)
            .map(TagOrContent::Content)
    }

    fn visit_i32<F>(self, value: i32) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_i32(value)
            .map(TagOrContent::Content)
    }

    fn visit_i64<F>(self, value: i64) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_i64(value)
            .map(TagOrContent::Content)
    }

    fn visit_u8<F>(self, value: u8) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_u8(value)
            .map(TagOrContent::Content)
    }

    fn visit_u16<F>(self, value: u16) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_u16(value)
            .map(TagOrContent::Content)
    }

    fn visit_u32<F>(self, value: u32) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_u32(value)
            .map(TagOrContent::Content)
    }

    fn visit_u64<F>(self, value: u64) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_u64(value)
            .map(TagOrContent::Content)
    }

    fn visit_f32<F>(self, value: f32) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_f32(value)
            .map(TagOrContent::Content)
    }

    fn visit_f64<F>(self, value: f64) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_f64(value)
            .map(TagOrContent::Content)
    }

    fn visit_char<F>(self, value: char) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_char(value)
            .map(TagOrContent::Content)
    }

    fn visit_str<F>(self, value: &str) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        if value == self.name {
            Ok(TagOrContent::Tag)
        } else {
            ContentVisitor::new()
                .visit_str(value)
                .map(TagOrContent::Content)
        }
    }

    fn visit_borrowed_str<F>(self, value: &'de str) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        if value == self.name {
            Ok(TagOrContent::Tag)
        } else {
            ContentVisitor::new()
                .visit_borrowed_str(value)
                .map(TagOrContent::Content)
        }
    }

    fn visit_string<F>(self, value: String) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        if value == self.name {
            Ok(TagOrContent::Tag)
        } else {
            ContentVisitor::new()
                .visit_string(value)
                .map(TagOrContent::Content)
        }
    }

    fn visit_bytes<F>(self, value: &[u8]) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        if value == self.name.as_bytes() {
            Ok(TagOrContent::Tag)
        } else {
            ContentVisitor::new()
                .visit_bytes(value)
                .map(TagOrContent::Content)
        }
    }

    fn visit_borrowed_bytes<F>(self, value: &'de [u8]) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        if value == self.name.as_bytes() {
            Ok(TagOrContent::Tag)
        } else {
            ContentVisitor::new()
                .visit_borrowed_bytes(value)
                .map(TagOrContent::Content)
        }
    }

    fn visit_byte_buf<F>(self, value: Vec<u8>) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        if value == self.name.as_bytes() {
            Ok(TagOrContent::Tag)
        } else {
            ContentVisitor::new()
                .visit_byte_buf(value)
                .map(TagOrContent::Content)
        }
    }

    fn visit_none<F>(self) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_none()
            .map(TagOrContent::Content)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
    {
        ContentVisitor::new()
            .visit_some(deserializer)
            .map(TagOrContent::Content)
    }

    fn visit_unit<F>(self) -> Result<Self::Value, F>
        where
            F: de::Error,
    {
        ContentVisitor::new()
            .visit_unit()
            .map(TagOrContent::Content)
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
    {
        ContentVisitor::new()
            .visit_newtype_struct(deserializer)
            .map(TagOrContent::Content)
    }

    fn visit_seq<V>(self, visitor: V) -> Result<Self::Value, V::Error>
        where
            V: SeqAccess<'de>,
    {
        ContentVisitor::new()
            .visit_seq(visitor)
            .map(TagOrContent::Content)
    }

    fn visit_map<V>(self, visitor: V) -> Result<Self::Value, V::Error>
        where
            V: MapAccess<'de>,
    {
        ContentVisitor::new()
            .visit_map(visitor)
            .map(TagOrContent::Content)
    }

    fn visit_enum<V>(self, visitor: V) -> Result<Self::Value, V::Error>
        where
            V: EnumAccess<'de>,
    {
        ContentVisitor::new()
            .visit_enum(visitor)
            .map(TagOrContent::Content)
    }
}

pub struct OptionallyTaggedContent<'de, T> {
    pub tag: Option<T>,
    pub content: Content<'de>,
}

pub struct OptionallyTaggedContentVisitor<'de, T> {
    tag_name: &'static str,
    value: PhantomData<OptionallyTaggedContent<'de, T>>,
}

impl<'de, T> OptionallyTaggedContentVisitor<'de, T> {
    /// Visitor for the content of an internally tagged enum with the given
    /// tag name.
    pub fn new(name: &'static str) -> Self {
        OptionallyTaggedContentVisitor {
            tag_name: name,
            value: PhantomData,
        }
    }
}

impl<'de, T> DeserializeSeed<'de> for OptionallyTaggedContentVisitor<'de, T>
    where
        T: Deserialize<'de>,
{
    type Value = OptionallyTaggedContent<'de, T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
    {
        // Internally tagged enums are only supported in self-describing
        // formats.
        deserializer.deserialize_any(self)
    }
}

impl<'de, T> Visitor<'de> for OptionallyTaggedContentVisitor<'de, T>
    where
        T: Deserialize<'de>,
{
    type Value = OptionallyTaggedContent<'de, T>;

    fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str("optionally internally tagged enum")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
    {
        let tag = match seq.next_element()? {
            Some(tag) => tag,
            None => {
                return Err(de::Error::missing_field(self.tag_name));
            }
        };
        let rest = de::value::SeqAccessDeserializer::new(seq);
        Ok(OptionallyTaggedContent {
            tag,
            content: Content::deserialize(rest)?,
        })
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
    {
        let mut tag = None;
        let mut vec = Vec::with_capacity(size_hint::cautious(map.size_hint()));
        while let Some(k) = map.next_key_seed(TagOrContentVisitor::new(self.tag_name))? {
            match k {
                TagOrContent::Tag => {
                    if tag.is_some() {
                        return Err(de::Error::duplicate_field(self.tag_name));
                    }
                    tag = Some(map.next_value()?);
                }
                TagOrContent::Content(k) => {
                    let v = map.next_value()?;
                    vec.push((k, v));
                }
            }
        }
        Ok(OptionallyTaggedContent {
            tag,
            content: Content::Map(vec),
        })
    }
}
