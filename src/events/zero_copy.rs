//! Defines zero-copy XML events used throughout this library.
//!
//! A XML event often represents part of a XML element.
//! They occur both during reading and writing and are
//! usually used with the stream-oriented API.
//!
//! For example, the XML element
//! ```xml
//! <name attr="value">Inner text</name>
//! ```
//! consists of the three events `Start`, `Text` and `End`.
//! They can also represent other parts in an XML document like the
//! XML declaration. Each Event usually contains further information,
//! like the tag name, the attribute or the inner text.
//!
//! See [`Event`] for a list of all possible events.
//!
//! # Reading
//! When reading a XML stream, the events are emitted by [`Reader::read_event`]
//! and [`Reader::read_event_into`]. You must listen
//! for the different types of events you are interested in.
//!
//! See [`Reader`] for further information.
//!
//! # Writing
//! When writing the XML document, you must create the XML element
//! by constructing the events it consists of and pass them to the writer
//! sequentially.
//!
//! See [`Writer`] for further information.
//!
//! [`Reader::read_event`]: crate::reader::Reader::read_event
//! [`Reader::read_event_into`]: crate::reader::Reader::read_event_into
//! [`Reader`]: crate::reader::Reader
//! [`Writer`]: crate::writer::Writer
//! [`Event`]: crate::events::Event

#[cfg(feature = "encoding")]
use encoding_rs::Encoding;
use std::borrow::Cow;
use std::fmt::{self, Debug, Formatter};
use std::ops::Deref;
use std::str::from_utf8;

use crate::encoding::Decoder;
use crate::errors::{Error, IllFormedError};
use crate::events::attributes::{AttrError, Attribute, Attributes};
use crate::events::{BytesCData, BytesEnd, BytesRef, BytesText};
use crate::name::{LocalName, QName};
use crate::utils::write_byte_string;

/// Opening tag data (`Event::Start`), with optional attributes: `<name attr="value">`.
///
/// The name can be accessed using the [`name`] or [`local_name`] methods.
/// An iterator over the attributes is returned by the [`attributes`] method.
///
/// This event implements `Deref<Target = [u8]>`. The `deref()` implementation
/// returns the content of this event between `<` and `>` or `/>`:
///
/// ```
/// # use quick_xml::events::{BytesStart, Event};
/// # use quick_xml::reader::Reader;
/// # use pretty_assertions::assert_eq;
/// // Remember, that \ at the end of string literal strips
/// // all space characters to the first non-space character
/// let mut reader = Reader::from_str("\
///     <element a1 = 'val1' a2=\"val2\" />\
///     <element a1 = 'val1' a2=\"val2\" >"
/// );
/// let content = "element a1 = 'val1' a2=\"val2\" ";
/// let event = BytesStart::from_content(content, 7);
///
/// assert_eq!(reader.read_event().unwrap(), Event::Empty(event.borrow()));
/// assert_eq!(reader.read_event().unwrap(), Event::Start(event.borrow()));
/// // deref coercion of &BytesStart to &[u8]
/// assert_eq!(&event as &[u8], content.as_bytes());
/// // AsRef<[u8]> for &T + deref coercion
/// assert_eq!(event.as_ref(), content.as_bytes());
/// ```
///
/// [`name`]: Self::name
/// [`local_name`]: Self::local_name
/// [`attributes`]: Self::attributes
#[derive(Clone, Eq, PartialEq)]
pub struct BytesStartRef<'a> {
    /// content of the element, before any utf8 conversion
    pub(crate) buf: &'a [u8],
    /// end of the element name, the name starts at that the start of `buf`
    pub(crate) name_len: usize,
    /// Encoding used for `buf`
    decoder: Decoder,
}

impl<'a> BytesStartRef<'a> {
    /// Internal constructor, used by `Reader`. Supplies data in reader's encoding
    #[inline]
    pub const fn wrap(content: &'a [u8], name_len: usize, decoder: Decoder) -> Self {
        BytesStartRef {
            buf: content,
            name_len,
            decoder,
        }
    }

    /// Creates a new `BytesStart` from the given name.
    ///
    /// # Warning
    ///
    /// `name` must be a valid name.
    #[inline]
    pub fn new(name: &'a str) -> Self {
        BytesStartRef {
            name_len: name.len(),
            buf: name.as_bytes(),
            decoder: Decoder::utf8(),
        }
    }

    /// Creates a new `BytesStart` from the given content (name + attributes).
    ///
    /// # Warning
    ///
    /// `&content[..name_len]` must be a valid name, and the remainder of `content`
    /// must be correctly-formed attributes. Neither are checked, it is possible
    /// to generate invalid XML if `content` or `name_len` are incorrect.
    #[inline]
    pub fn from_content(content: &'a str, name_len: usize) -> Self {
        BytesStartRef {
            buf: content.as_bytes(),
            name_len,
            decoder: Decoder::utf8(),
        }
    }

    // /// Converts the event into an owned event.
    // pub fn into_owned(self) -> BytesStart<'static> {
    //     BytesStart {
    //         buf: Cow::Owned(self.buf.into_owned()),
    //         name_len: self.name_len,
    //         decoder: self.decoder,
    //     }
    // }

    // /// Converts the event into an owned event without taking ownership of Event
    // pub fn to_owned(&self) -> BytesStart<'static> {
    //     BytesStart {
    //         buf: Cow::Owned(self.buf.clone().into_owned()),
    //         name_len: self.name_len,
    //         decoder: self.decoder,
    //     }
    // }

    // /// Converts the event into a borrowed event. Most useful when paired with [`to_end`].
    // ///
    // /// # Example
    // ///
    // /// ```
    // /// use quick_xml::events::{BytesStart, Event};
    // /// # use quick_xml::writer::Writer;
    // /// # use quick_xml::Error;
    // ///
    // /// struct SomeStruct<'a> {
    // ///     attrs: BytesStart<'a>,
    // ///     // ...
    // /// }
    // /// # impl<'a> SomeStruct<'a> {
    // /// # fn example(&self) -> Result<(), Error> {
    // /// # let mut writer = Writer::new(Vec::new());
    // ///
    // /// writer.write_event(Event::Start(self.attrs.borrow()))?;
    // /// // ...
    // /// writer.write_event(Event::End(self.attrs.to_end()))?;
    // /// # Ok(())
    // /// # }}
    // /// ```
    // ///
    // /// [`to_end`]: Self::to_end
    // pub fn borrow(&self) -> BytesStart<'_> {
    //     BytesStart {
    //         buf: Cow::Borrowed(&self.buf),
    //         name_len: self.name_len,
    //         decoder: self.decoder,
    //     }
    // }

    /// Creates new paired close tag
    #[inline]
    pub fn to_end(&self) -> BytesEnd<'_> {
        BytesEnd::from(self.name())
    }

    /// Get the decoder, used to decode bytes, read by the reader which produces
    /// this event, to the strings.
    ///
    /// When event was created manually, encoding is UTF-8.
    ///
    /// If [`encoding`] feature is enabled and no encoding is specified in declaration,
    /// defaults to UTF-8.
    ///
    /// [`encoding`]: ../index.html#encoding
    #[inline]
    pub const fn decoder(&self) -> Decoder {
        self.decoder
    }

    /// Gets the undecoded raw tag name, as present in the input stream.
    #[inline]
    pub fn name(&self) -> QName<'_> {
        QName(&self.buf[..self.name_len])
    }

    /// Gets the undecoded raw local tag name (excluding namespace) as present
    /// in the input stream.
    ///
    /// All content up to and including the first `:` character is removed from the tag name.
    #[inline]
    pub fn local_name(&self) -> LocalName<'_> {
        self.name().into()
    }
}

/// Attribute-related methods
impl<'a> BytesStartRef<'a> {
    // /// Consumes `self` and yield a new `BytesStart` with additional attributes from an iterator.
    // ///
    // /// The yielded items must be convertible to [`Attribute`] using `Into`.
    // pub fn with_attributes<'b, I>(mut self, attributes: I) -> Self
    // where
    //     I: IntoIterator,
    //     I::Item: Into<Attribute<'b>>,
    // {
    //     self.extend_attributes(attributes);
    //     self
    // }

    // /// Add additional attributes to this tag using an iterator.
    // ///
    // /// The yielded items must be convertible to [`Attribute`] using `Into`.
    // pub fn extend_attributes<'b, I>(&mut self, attributes: I) -> &mut BytesStart<'a>
    // where
    //     I: IntoIterator,
    //     I::Item: Into<Attribute<'b>>,
    // {
    //     for attr in attributes {
    //         self.push_attribute(attr);
    //     }
    //     self
    // }

    // /// Adds an attribute to this element.
    // pub fn push_attribute<'b, A>(&mut self, attr: A)
    // where
    //     A: Into<Attribute<'b>>,
    // {
    //     self.buf.to_mut().push(b' ');
    //     self.push_attr(attr.into());
    // }

    // /// Remove all attributes from the ByteStart
    // pub fn clear_attributes(&mut self) -> &mut BytesStart<'a> {
    //     self.buf.to_mut().truncate(self.name_len);
    //     self
    // }

    /// Returns an iterator over the attributes of this tag.
    pub fn attributes(&self) -> Attributes<'a> {
        Attributes::wrap(self.buf, self.name_len, false, self.decoder)
    }

    /// Returns an iterator over the HTML-like attributes of this tag (no mandatory quotes or `=`).
    pub fn html_attributes(self) -> Attributes<'a> {
        Attributes::wrap(self.buf, self.name_len, true, self.decoder)
    }

    /// Gets the undecoded raw string with the attributes of this tag as a `&[u8]`,
    /// including the whitespace after the tag name if there is any.
    #[inline]
    pub fn attributes_raw(&self) -> &[u8] {
        &self.buf[self.name_len..]
    }

    /// Try to get an attribute
    pub fn try_get_attribute<N: AsRef<[u8]> + Sized>(
        &self,
        attr_name: N,
    ) -> Result<Option<Attribute<'a>>, AttrError> {
        for a in self.attributes().with_checks(false) {
            let a = a?;
            if a.key.as_ref() == attr_name.as_ref() {
                return Ok(Some(a));
            }
        }
        Ok(None)
    }

    // /// Adds an attribute to this element.
    // pub(crate) fn push_attr<'b>(&mut self, attr: Attribute<'b>) {
    //     let bytes = self.buf.to_mut();
    //     bytes.extend_from_slice(attr.key.as_ref());
    //     bytes.extend_from_slice(b"=\"");
    //     // FIXME: need to escape attribute content
    //     bytes.extend_from_slice(attr.value.as_ref());
    //     bytes.push(b'"');
    // }

    // /// Adds new line in existing element
    // pub(crate) fn push_newline(&mut self) {
    //     self.buf.to_mut().push(b'\n');
    // }

    // /// Adds indentation bytes in existing element
    // pub(crate) fn push_indent(&mut self, indent: &[u8]) {
    //     self.buf.to_mut().extend_from_slice(indent);
    // }
}

impl<'a> Debug for BytesStartRef<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesStart {{ buf: ")?;
        write_byte_string(f, self.buf)?;
        write!(f, ", name_len: {} }}", self.name_len)
    }
}

impl<'a> Deref for BytesStartRef<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.buf
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for BytesStartRef<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let s = <&str>::arbitrary(u)?;
        if s.is_empty() || !s.chars().all(char::is_alphanumeric) {
            return Err(arbitrary::Error::IncorrectFormat);
        }
        unimplemented!("No valid arbitrary implementation");
        // let mut result = Self::new(s);
        // result.extend_attributes(Vec::<(&str, &str)>::arbitrary(u)?.into_iter());
        // Ok(result)
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        <&str as arbitrary::Arbitrary>::size_hint(depth)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// [Processing instructions][PI] (PIs) allow documents to contain instructions for applications.
///
/// This event implements `Deref<Target = [u8]>`. The `deref()` implementation
/// returns the content of this event between `<?` and `?>`.
///
/// Note, that inner text will not contain `?>` sequence inside:
///
/// ```
/// # use quick_xml::events::{BytesPI, Event};
/// # use quick_xml::reader::Reader;
/// # use pretty_assertions::assert_eq;
/// let mut reader = Reader::from_str("<?processing instruction >:-<~ ?>");
/// let content = "processing instruction >:-<~ ";
/// let event = BytesPI::new(content);
///
/// assert_eq!(reader.read_event().unwrap(), Event::PI(event.borrow()));
/// // deref coercion of &BytesPI to &[u8]
/// assert_eq!(&event as &[u8], content.as_bytes());
/// // AsRef<[u8]> for &T + deref coercion
/// assert_eq!(event.as_ref(), content.as_bytes());
/// ```
///
/// [PI]: https://www.w3.org/TR/xml11/#sec-pi
#[derive(Clone, Eq, PartialEq)]
pub struct BytesPIRef<'a> {
    content: BytesStartRef<'a>,
}

impl<'a> BytesPIRef<'a> {
    /// Creates a new `BytesPI` from a byte sequence in the specified encoding.
    #[inline]
    pub(crate) const fn wrap(content: &'a [u8], target_len: usize, decoder: Decoder) -> Self {
        Self {
            content: BytesStartRef::wrap(content, target_len, decoder),
        }
    }

    /// Creates a new `BytesPI` from a string.
    ///
    /// # Warning
    ///
    /// `content` must not contain the `?>` sequence.
    #[inline]
    pub fn new(content: &'a str) -> Self {
        // let buf = str_cow_to_bytes(content);
        // let name_len = name_len(&buf);
        Self {
            content: BytesStartRef::new(content),
        }
    }

    // /// Ensures that all data is owned to extend the object's lifetime if
    // /// necessary.
    // #[inline]
    // pub fn into_owned(self) -> BytesPI<'static> {
    //     BytesPI {
    //         content: self.content.into_owned().into(),
    //     }
    // }

    /// Extracts the inner `Cow` from the `BytesPI` event container.
    #[inline]
    pub fn into_inner(self) -> &'a [u8] {
        self.content.buf
    }

    // /// Converts the event into a borrowed event.
    // #[inline]
    // pub fn borrow(&self) -> BytesPI<'_> {
    //     BytesPI {
    //         content: self.content.borrow(),
    //     }
    // }

    /// A target used to identify the application to which the instruction is directed.
    ///
    /// # Example
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::BytesPI;
    ///
    /// let instruction = BytesPI::new(r#"xml-stylesheet href="style.css""#);
    /// assert_eq!(instruction.target(), b"xml-stylesheet");
    /// ```
    #[inline]
    pub fn target(&self) -> &[u8] {
        self.content.name().0
    }

    /// Content of the processing instruction. Contains everything between target
    /// name and the end of the instruction. A direct consequence is that the first
    /// character is always a space character.
    ///
    /// # Example
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::BytesPI;
    ///
    /// let instruction = BytesPI::new(r#"xml-stylesheet href="style.css""#);
    /// assert_eq!(instruction.content(), br#" href="style.css""#);
    /// ```
    #[inline]
    pub fn content(&self) -> &[u8] {
        self.content.attributes_raw()
    }

    /// A view of the processing instructions' content as a list of key-value pairs.
    ///
    /// Key-value pairs are used in some processing instructions, for example in
    /// `<?xml-stylesheet?>`.
    ///
    /// Returned iterator does not validate attribute values as may required by
    /// target's rules. For example, it doesn't check that substring `?>` is not
    /// present in the attribute value. That shouldn't be the problem when event
    /// is produced by the reader, because reader detects end of processing instruction
    /// by the first `?>` sequence, as required by the specification, and therefore
    /// this sequence cannot appear inside it.
    ///
    /// # Example
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use std::borrow::Cow;
    /// use quick_xml::events::attributes::Attribute;
    /// use quick_xml::events::BytesPI;
    /// use quick_xml::name::QName;
    ///
    /// let instruction = BytesPI::new(r#"xml-stylesheet href="style.css""#);
    /// for attr in instruction.attributes() {
    ///     assert_eq!(attr, Ok(Attribute {
    ///         key: QName(b"href"),
    ///         value: Cow::Borrowed(b"style.css"),
    ///     }));
    /// }
    /// ```
    #[inline]
    pub fn attributes(&self) -> Attributes<'_> {
        self.content.attributes()
    }
}

impl<'a> Debug for BytesPIRef<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesPI {{ content: ")?;
        write_byte_string(f, &self.content.buf)?;
        write!(f, " }}")
    }
}

impl<'a> Deref for BytesPIRef<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.content
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for BytesPIRef<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::new(<&str>::arbitrary(u)?))
    }
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        <&str as arbitrary::Arbitrary>::size_hint(depth)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// An XML declaration (`Event::Decl`).
///
/// [W3C XML 1.1 Prolog and Document Type Declaration](http://w3.org/TR/xml11/#sec-prolog-dtd)
///
/// This event implements `Deref<Target = [u8]>`. The `deref()` implementation
/// returns the content of this event between `<?` and `?>`.
///
/// Note, that inner text will not contain `?>` sequence inside:
///
/// ```
/// # use quick_xml::events::{BytesDecl, BytesStart, Event};
/// # use quick_xml::reader::Reader;
/// # use pretty_assertions::assert_eq;
/// let mut reader = Reader::from_str("<?xml version = '1.0' ?>");
/// let content = "xml version = '1.0' ";
/// let event = BytesDecl::from_start(BytesStart::from_content(content, 3));
///
/// assert_eq!(reader.read_event().unwrap(), Event::Decl(event.borrow()));
/// // deref coercion of &BytesDecl to &[u8]
/// assert_eq!(&event as &[u8], content.as_bytes());
/// // AsRef<[u8]> for &T + deref coercion
/// assert_eq!(event.as_ref(), content.as_bytes());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BytesDeclRef<'a> {
    content: BytesStartRef<'a>,
}

impl<'a> BytesDeclRef<'a> {
    // /// Constructs a new `XmlDecl` from the (mandatory) _version_ (should be `1.0` or `1.1`),
    // /// the optional _encoding_ (e.g., `UTF-8`) and the optional _standalone_ (`yes` or `no`)
    // /// attribute.
    // ///
    // /// Does not escape any of its inputs. Always uses double quotes to wrap the attribute values.
    // /// The caller is responsible for escaping attribute values. Shouldn't usually be relevant since
    // /// the double quote character is not allowed in any of the attribute values.
    // pub fn new(
    //     version: &str,
    //     encoding: Option<&str>,
    //     standalone: Option<&str>,
    // ) -> BytesDecl<'static> {
    //     // Compute length of the buffer based on supplied attributes
    //     // ' encoding=""'   => 12
    //     let encoding_attr_len = if let Some(xs) = encoding {
    //         12 + xs.len()
    //     } else {
    //         0
    //     };
    //     // ' standalone=""' => 14
    //     let standalone_attr_len = if let Some(xs) = standalone {
    //         14 + xs.len()
    //     } else {
    //         0
    //     };
    //     // 'xml version=""' => 14
    //     let mut buf = String::with_capacity(14 + encoding_attr_len + standalone_attr_len);

    //     buf.push_str("xml version=\"");
    //     buf.push_str(version);

    //     if let Some(encoding_val) = encoding {
    //         buf.push_str("\" encoding=\"");
    //         buf.push_str(encoding_val);
    //     }

    //     if let Some(standalone_val) = standalone {
    //         buf.push_str("\" standalone=\"");
    //         buf.push_str(standalone_val);
    //     }
    //     buf.push('"');

    //     BytesDecl {
    //         content: BytesStart::from_content(buf, 3),
    //     }
    // }

    /// Creates a `BytesDecl` from a `BytesStart`
    pub const fn from_start(start: BytesStartRef<'a>) -> Self {
        Self { content: start }
    }

    /// Gets xml version, excluding quotes (`'` or `"`).
    ///
    /// According to the [grammar], the version *must* be the first thing in the declaration.
    /// This method tries to extract the first thing in the declaration and return it.
    /// In case of multiple attributes value of the first one is returned.
    ///
    /// If version is missed in the declaration, or the first thing is not a version,
    /// [`IllFormedError::MissingDeclVersion`] will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use quick_xml::errors::{Error, IllFormedError};
    /// use quick_xml::events::{BytesDecl, BytesStart};
    ///
    /// // <?xml version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" version='1.1'", 0));
    /// assert_eq!(decl.version().unwrap(), b"1.1".as_ref());
    ///
    /// // <?xml version='1.0' version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" version='1.0' version='1.1'", 0));
    /// assert_eq!(decl.version().unwrap(), b"1.0".as_ref());
    ///
    /// // <?xml encoding='utf-8'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" encoding='utf-8'", 0));
    /// match decl.version() {
    ///     Err(Error::IllFormed(IllFormedError::MissingDeclVersion(Some(key)))) => assert_eq!(key, "encoding"),
    ///     _ => assert!(false),
    /// }
    ///
    /// // <?xml encoding='utf-8' version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" encoding='utf-8' version='1.1'", 0));
    /// match decl.version() {
    ///     Err(Error::IllFormed(IllFormedError::MissingDeclVersion(Some(key)))) => assert_eq!(key, "encoding"),
    ///     _ => assert!(false),
    /// }
    ///
    /// // <?xml?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content("", 0));
    /// match decl.version() {
    ///     Err(Error::IllFormed(IllFormedError::MissingDeclVersion(None))) => {},
    ///     _ => assert!(false),
    /// }
    /// ```
    ///
    /// [grammar]: https://www.w3.org/TR/xml11/#NT-XMLDecl
    pub fn version(&self) -> Result<Cow<'_, [u8]>, Error> {
        // The version *must* be the first thing in the declaration.
        match self.content.attributes().with_checks(false).next() {
            Some(Ok(a)) if a.key.as_ref() == b"version" => Ok(a.value),
            // first attribute was not "version"
            Some(Ok(a)) => {
                let found = from_utf8(a.key.as_ref())
                    .map_err(|_| IllFormedError::MissingDeclVersion(None))?
                    .to_string();
                Err(Error::IllFormed(IllFormedError::MissingDeclVersion(Some(
                    found,
                ))))
            }
            // error parsing attributes
            Some(Err(e)) => Err(e.into()),
            // no attributes
            None => Err(Error::IllFormed(IllFormedError::MissingDeclVersion(None))),
        }
    }

    /// Gets xml encoding, excluding quotes (`'` or `"`).
    ///
    /// Although according to the [grammar] encoding must appear before `"standalone"`
    /// and after `"version"`, this method does not check that. The first occurrence
    /// of the attribute will be returned even if there are several. Also, method does
    /// not restrict symbols that can forming the encoding, so the returned encoding
    /// name may not correspond to the grammar.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::borrow::Cow;
    /// use quick_xml::Error;
    /// use quick_xml::events::{BytesDecl, BytesStart};
    ///
    /// // <?xml version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" version='1.1'", 0));
    /// assert!(decl.encoding().is_none());
    ///
    /// // <?xml encoding='utf-8'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" encoding='utf-8'", 0));
    /// match decl.encoding() {
    ///     Some(Ok(Cow::Borrowed(encoding))) => assert_eq!(encoding, b"utf-8"),
    ///     _ => assert!(false),
    /// }
    ///
    /// // <?xml encoding='something_WRONG' encoding='utf-8'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" encoding='something_WRONG' encoding='utf-8'", 0));
    /// match decl.encoding() {
    ///     Some(Ok(Cow::Borrowed(encoding))) => assert_eq!(encoding, b"something_WRONG"),
    ///     _ => assert!(false),
    /// }
    /// ```
    ///
    /// [grammar]: https://www.w3.org/TR/xml11/#NT-XMLDecl
    pub fn encoding(&self) -> Option<Result<Cow<'a, [u8]>, AttrError>> {
        self.content
            .try_get_attribute("encoding")
            .map(|a| a.map(|a| a.value))
            .transpose()
    }

    /// Gets xml standalone, excluding quotes (`'` or `"`).
    ///
    /// Although according to the [grammar] standalone flag must appear after `"version"`
    /// and `"encoding"`, this method does not check that. The first occurrence of the
    /// attribute will be returned even if there are several. Also, method does not
    /// restrict symbols that can forming the value, so the returned flag name may not
    /// correspond to the grammar.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::borrow::Cow;
    /// use quick_xml::Error;
    /// use quick_xml::events::{BytesDecl, BytesStart};
    ///
    /// // <?xml version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" version='1.1'", 0));
    /// assert!(decl.standalone().is_none());
    ///
    /// // <?xml standalone='yes'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" standalone='yes'", 0));
    /// match decl.standalone() {
    ///     Some(Ok(Cow::Borrowed(encoding))) => assert_eq!(encoding, b"yes"),
    ///     _ => assert!(false),
    /// }
    ///
    /// // <?xml standalone='something_WRONG' encoding='utf-8'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" standalone='something_WRONG' encoding='utf-8'", 0));
    /// match decl.standalone() {
    ///     Some(Ok(Cow::Borrowed(flag))) => assert_eq!(flag, b"something_WRONG"),
    ///     _ => assert!(false),
    /// }
    /// ```
    ///
    /// [grammar]: https://www.w3.org/TR/xml11/#NT-XMLDecl
    pub fn standalone(&self) -> Option<Result<Cow<'_, [u8]>, AttrError>> {
        self.content
            .try_get_attribute("standalone")
            .map(|a| a.map(|a| a.value))
            .transpose()
    }

    /// Gets the actual encoding using [_get an encoding_](https://encoding.spec.whatwg.org/#concept-encoding-get)
    /// algorithm.
    ///
    /// If encoding in not known, or `encoding` key was not found, returns `None`.
    /// In case of duplicated `encoding` key, encoding, corresponding to the first
    /// one, is returned.
    #[cfg(feature = "encoding")]
    pub fn encoder(&self) -> Option<&'static Encoding> {
        self.encoding()
            .and_then(|e| e.ok())
            .and_then(|e| Encoding::for_label(&e))
    }

    // /// Converts the event into an owned event.
    // pub fn into_owned(self) -> BytesDecl<'static> {
    //     BytesDecl {
    //         content: self.content.into_owned(),
    //     }
    // }

    // /// Converts the event into a borrowed event.
    // #[inline]
    // pub fn borrow(&self) -> BytesDecl<'_> {
    //     BytesDecl {
    //         content: self.content.borrow(),
    //     }
    // }
}

impl<'a> Deref for BytesDeclRef<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.content
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for BytesDeclRef<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        unimplemented!();
        // Ok(Self::new(
        //     <&str>::arbitrary(u)?,
        //     Option::<&str>::arbitrary(u)?,
        //     Option::<&str>::arbitrary(u)?,
        // ))
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        <&str as arbitrary::Arbitrary>::size_hint(depth)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Event emitted by [`Reader::read_event_into`].
///
/// [`Reader::read_event_into`]: crate::reader::Reader::read_event_into
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum EventRef<'a> {
    /// Start tag (with attributes) `<tag attr="value">`.
    Start(BytesStartRef<'a>),
    /// End tag `</tag>`.
    End(BytesEnd<'a>),
    /// Empty element tag (with attributes) `<tag attr="value" />`.
    Empty(BytesStartRef<'a>),
    /// Escaped character data between tags.
    Text(BytesText<'a>),
    /// Unescaped character data stored in `<![CDATA[...]]>`.
    CData(BytesCData<'a>),
    /// Comment `<!-- ... -->`.
    Comment(BytesText<'a>),
    /// XML declaration `<?xml ...?>`.
    Decl(BytesDeclRef<'a>),
    /// Processing instruction `<?...?>`.
    PI(BytesPIRef<'a>),
    /// Document type definition data (DTD) stored in `<!DOCTYPE ...>`.
    DocType(BytesText<'a>),
    /// General reference `&entity;` in the textual data. Can be either an entity
    /// reference, or a character reference.
    GeneralRef(BytesRef<'a>),
    /// End of XML document.
    Eof,
}

impl<'a> EventRef<'a> {
    // /// Converts the event to an owned version, untied to the lifetime of
    // /// buffer used when reading but incurring a new, separate allocation.
    // pub fn into_owned(self) -> Event<'static> {
    //     match self {
    //         Event::Start(e) => Event::Start(e.into_owned()),
    //         Event::End(e) => Event::End(e.into_owned()),
    //         Event::Empty(e) => Event::Empty(e.into_owned()),
    //         Event::Text(e) => Event::Text(e.into_owned()),
    //         Event::Comment(e) => Event::Comment(e.into_owned()),
    //         Event::CData(e) => Event::CData(e.into_owned()),
    //         Event::Decl(e) => Event::Decl(e.into_owned()),
    //         Event::PI(e) => Event::PI(e.into_owned()),
    //         Event::DocType(e) => Event::DocType(e.into_owned()),
    //         Event::GeneralRef(e) => Event::GeneralRef(e.into_owned()),
    //         Event::Eof => Event::Eof,
    //     }
    // }

    // /// Converts the event into a borrowed event.
    // #[inline]
    // pub fn borrow(&self) -> Event<'_> {
    //     match self {
    //         Event::Start(e) => Event::Start(e.borrow()),
    //         Event::End(e) => Event::End(e.borrow()),
    //         Event::Empty(e) => Event::Empty(e.borrow()),
    //         Event::Text(e) => Event::Text(e.borrow()),
    //         Event::Comment(e) => Event::Comment(e.borrow()),
    //         Event::CData(e) => Event::CData(e.borrow()),
    //         Event::Decl(e) => Event::Decl(e.borrow()),
    //         Event::PI(e) => Event::PI(e.borrow()),
    //         Event::DocType(e) => Event::DocType(e.borrow()),
    //         Event::GeneralRef(e) => Event::GeneralRef(e.borrow()),
    //         Event::Eof => Event::Eof,
    //     }
    // }
}

impl<'a> Deref for EventRef<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match *self {
            EventRef::Start(ref e) | EventRef::Empty(ref e) => e,
            EventRef::End(ref e) => e,
            EventRef::Text(ref e) => e,
            EventRef::Decl(ref e) => e,
            EventRef::PI(ref e) => e,
            EventRef::CData(ref e) => e,
            EventRef::Comment(ref e) => e,
            EventRef::DocType(ref e) => e,
            EventRef::GeneralRef(ref e) => e,
            EventRef::Eof => &[],
        }
    }
}

impl<'a> AsRef<EventRef<'a>> for EventRef<'a> {
    fn as_ref(&self) -> &EventRef<'a> {
        self
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

#[inline]
fn str_cow_to_bytes<'a, C: Into<Cow<'a, str>>>(content: C) -> Cow<'a, [u8]> {
    match content.into() {
        Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
        Cow::Owned(s) => Cow::Owned(s.into_bytes()),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn bytestart_create() {
        let b = BytesStartRef::new("test");
        assert_eq!(b.len(), 4);
        assert_eq!(b.name(), QName(b"test"));
    }

    // #[test]
    // fn bytestart_set_name() {
    //     let mut b = BytesStart::new("test");
    //     assert_eq!(b.len(), 4);
    //     assert_eq!(b.name(), QName(b"test"));
    //     assert_eq!(b.attributes_raw(), b"");
    //     b.push_attribute(("x", "a"));
    //     assert_eq!(b.len(), 10);
    //     assert_eq!(b.attributes_raw(), b" x=\"a\"");
    //     b.set_name(b"g");
    //     assert_eq!(b.len(), 7);
    //     assert_eq!(b.name(), QName(b"g"));
    // }

    // #[test]
    // fn bytestart_clear_attributes() {
    //     let mut b = BytesStart::new("test");
    //     b.push_attribute(("x", "y\"z"));
    //     b.push_attribute(("x", "y\"z"));
    //     b.clear_attributes();
    //     assert!(b.attributes().next().is_none());
    //     assert_eq!(b.len(), 4);
    //     assert_eq!(b.name(), QName(b"test"));
    // }
}
