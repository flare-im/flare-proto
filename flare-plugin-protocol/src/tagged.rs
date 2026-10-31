//! Reading the enums tagged by `kind`.
//!
//! serde's derive for an internally tagged enum first buffers the whole object
//! into its private `Content` tree and then reads the variant from that tree,
//! so every type a variant holds is compiled a second time against it. The
//! buffer keeps what a parser saw, duplicate keys and their order at every
//! depth, which is why bytes, text and readers still go through the derive.
//!
//! A `serde_json::Value` has nothing of that left to keep: its objects cannot
//! hold a key twice. For a value, these enums take the tag out of the object
//! and read the variant from the rest of it through `Value` itself, the path
//! every one of their inner types already takes everywhere else. The tag is
//! checked exactly as the derive checks it: missing, repeated, not a string,
//! or naming no variant. The inner types are then read by `Value`'s rules, as
//! they are outside these enums: a unit variant written `{"name": {}}` is
//! refused, and an array one element too long is reported in `Value`'s words.
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor, value::StringDeserializer};
use serde_json::{Map, Value};
use std::{any::TypeId, fmt, marker::PhantomData};

const TAG: &str = "kind";

/// Whether `D` is an owned or a borrowed `serde_json::Value`. The answer is a
/// constant of each instantiation, so the compiler keeps only the path taken.
pub(crate) fn is_json_value<'de, D: de::Deserializer<'de>>() -> bool {
    let id = type_id::<D>();
    id == TypeId::of::<Value>() || id == TypeId::of::<&'static Value>()
}

/// `TypeId::of` without its `'static` bound; lifetimes do not take part.
fn type_id<T: ?Sized>() -> TypeId {
    trait NonStaticAny {
        fn get_type_id(&self) -> TypeId
        where
            Self: 'static;
    }
    impl<T: ?Sized> NonStaticAny for PhantomData<T> {
        fn get_type_id(&self) -> TypeId
        where
            Self: 'static,
        {
            TypeId::of::<T>()
        }
    }
    let phantom = PhantomData::<T>;
    // SAFETY: only the lifetime bound of the trait object changes. The method
    // reads nothing through the reference, and a `TypeId` does not encode
    // lifetimes, so nothing relies on the lifetime being real.
    let erased = unsafe {
        std::mem::transmute::<&dyn NonStaticAny, &(dyn NonStaticAny + 'static)>(&phantom)
    };
    erased.get_type_id()
}

/// Reads a `kind`-tagged enum from a JSON value: takes the tag out, then
/// lets `read` (the enum's derive in its externally tagged form) read the
/// variant the tag names from the rest of the object, or of the array after
/// its first element.
///
/// For a value only: an object of another input may repeat a key, and the
/// rest keeps the last one where the internally tagged derive refuses it.
pub(crate) fn deserialize<'de, D, T>(
    deserializer: D,
    expecting: &'static str,
    read: impl FnOnce(ByKind) -> Result<T, serde_json::Error>,
) -> Result<T, D::Error>
where
    D: de::Deserializer<'de>,
{
    let tagged = deserializer.deserialize_any(Tagged { expecting })?;
    read(tagged).map_err(de::Error::custom)
}

struct Tagged {
    expecting: &'static str,
}
impl<'de> Visitor<'de> for Tagged {
    type Value = ByKind;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.expecting)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<ByKind, A::Error> {
        let mut kind = None;
        let mut rest = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if key == TAG {
                if kind.is_some() {
                    return Err(de::Error::duplicate_field(TAG));
                }
                kind = Some(map.next_value_seed(Kind)?);
            } else {
                rest.insert(key, map.next_value()?);
            }
        }
        let kind = kind.ok_or_else(|| de::Error::missing_field(TAG))?;
        Ok(ByKind {
            kind,
            rest: Value::Object(rest),
        })
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ByKind, A::Error> {
        let kind = seq
            .next_element_seed(Kind)?
            .ok_or_else(|| de::Error::missing_field(TAG))?;
        let mut rest = Vec::new();
        while let Some(value) = seq.next_element()? {
            rest.push(value);
        }
        Ok(ByKind {
            kind,
            rest: Value::Array(rest),
        })
    }
}

/// The tag, which must be a string as the derive reads a variant name.
struct Kind;
impl<'de> DeserializeSeed<'de> for Kind {
    type Value = String;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<String, D::Error> {
        deserializer.deserialize_identifier(self)
    }
}
impl Visitor<'_> for Kind {
    type Value = String;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("variant identifier")
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<String, E> {
        Ok(value.to_owned())
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<String, E> {
        Ok(value)
    }
}

/// A tagged value split into its tag and the rest, presented as the
/// externally tagged enum `{kind: rest}`.
pub(crate) struct ByKind {
    kind: String,
    rest: Value,
}
impl<'de> de::Deserializer<'de> for ByKind {
    type Error = serde_json::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_enum(self)
    }
    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum
        identifier ignored_any
    }
}
impl<'de> de::EnumAccess<'de> for ByKind {
    type Error = serde_json::Error;
    type Variant = Rest;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Rest), Self::Error> {
        let kind = StringDeserializer::<serde_json::Error>::new(self.kind);
        Ok((seed.deserialize(kind)?, Rest(self.rest)))
    }
}
/// What a variant is read from: the object without its tag, or the array
/// after it.
pub(crate) struct Rest(Value);
impl<'de> de::VariantAccess<'de> for Rest {
    type Error = serde_json::Error;

    /// Like the internally tagged derive, a unit variant ignores the rest.
    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Self::Error> {
        seed.deserialize(self.0)
    }
    fn tuple_variant<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, Self::Error> {
        de::Deserializer::deserialize_any(self.0, visitor)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        de::Deserializer::deserialize_any(self.0, visitor)
    }
}

/// `Deserialize` for an enum tagged by `kind`: serde's internally tagged
/// derive for every input but a JSON value, [`deserialize`] for a value.
///
/// The invocation lists every variant once, with its tag and its fields as
/// the enum declares them (field attributes included), followed by the
/// enum's own container attributes for deserialization. The enum itself
/// derives only `Serialize`.
macro_rules! kind_tagged {
    (
        $name:ident = $type_name:tt $(, $attr:tt)* {
            $($kind:tt => $variant:ident $fields:tt),+ $(,)?
        }
    ) => {
        const _: () = {
            use serde::{Deserialize, Deserializer};

            #[derive(Deserialize)]
            #[serde(remote = $type_name, tag = "kind" $(, $attr)*)]
            #[allow(dead_code, clippy::large_enum_variant)]
            enum Internally {
                $(#[serde(rename = $kind)] $variant $fields),+
            }

            #[derive(Deserialize)]
            #[serde(remote = $type_name $(, $attr)*)]
            #[allow(dead_code, clippy::large_enum_variant)]
            enum Externally {
                $(#[serde(rename = $kind)] $variant $fields),+
            }

            impl<'de> Deserialize<'de> for $name {
                fn deserialize<D: Deserializer<'de>>(
                    deserializer: D,
                ) -> ::std::result::Result<Self, D::Error> {
                    if $crate::tagged::is_json_value::<D>() {
                        $crate::tagged::deserialize(
                            deserializer,
                            concat!("internally tagged enum ", $type_name),
                            Externally::deserialize,
                        )
                    } else {
                        Internally::deserialize(deserializer)
                    }
                }
            }

            /// Fails to compile when the enum has a variant the list lacks.
            #[allow(dead_code)]
            fn every_variant_is_listed(value: &$name) {
                match value {
                    $($name::$variant { .. } => {})+
                }
            }
        };
    };
}
pub(crate) use kind_tagged;

#[cfg(test)]
mod tests {
    use super::*;

    fn tag_of(text: &str) -> Result<String, String> {
        let mut input = serde_json::Deserializer::from_str(text);
        deserialize(&mut input, "internally tagged enum T", |tagged| {
            Ok(tagged.kind)
        })
        .map_err(|error| error.to_string())
    }

    /// The tag rules do not depend on the input, though only a value is read
    /// this way: a value cannot repeat the tag, text can.
    #[test]
    fn the_tag_must_be_present_once_and_a_string() {
        assert_eq!(tag_of(r#"{"x":1,"kind":"a"}"#), Ok("a".into()));
        assert_eq!(tag_of(r#"["a",1]"#), Ok("a".into()));
        assert_eq!(
            tag_of(r#"{"kind":"a","kind":"a"}"#),
            Err("duplicate field `kind` at line 1 column 18".into())
        );
        assert_eq!(
            tag_of(r#"{"x":1}"#),
            Err("missing field `kind` at line 1 column 7".into())
        );
        assert_eq!(
            tag_of(r#"[]"#),
            Err("missing field `kind` at line 1 column 2".into())
        );
        assert_eq!(
            tag_of(r#"{"kind":1}"#),
            Err("invalid type: integer `1`, expected variant identifier at line 1 column 9".into())
        );
        assert_eq!(
            tag_of(r#""a""#),
            Err(
                "invalid type: string \"a\", expected internally tagged enum T at line 1 column 3"
                    .into()
            )
        );
    }
}
