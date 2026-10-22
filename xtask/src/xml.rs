// Namespace-aware update XML parser reused from BibCiTeX's xtask.
use crate::{Result, ensure};
use quick_xml::{XmlVersion, events::Event, name::ResolveResult, reader::NsReader};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Element {
    pub(crate) namespace: String,
    pub(crate) name: String,
    attrs: BTreeMap<(String, String), String>,
    pub(crate) children: Vec<Element>,
    pub(crate) text: String,
}

impl Element {
    pub(crate) fn attr(&self, namespace: &str, name: &str) -> &str {
        self.attrs
            .get(&(namespace.to_owned(), name.to_owned()))
            .map_or("", String::as_str)
    }
    pub(crate) fn one(&self, namespace: &str, name: &str) -> Result<&Self> {
        let found = self
            .children
            .iter()
            .filter(|node| node.name == name && node.namespace == namespace)
            .collect::<Vec<_>>();
        ensure(found.len() == 1, &format!("Expected exactly one {name}"))?;
        Ok(found[0])
    }
}

fn namespace(resolved: ResolveResult<'_>) -> Result<String> {
    match resolved {
        ResolveResult::Bound(value) => Ok(value.0.to_owned()),
        ResolveResult::Unbound => Ok(String::new()),
        ResolveResult::Unknown(_) => Err("Undeclared XML namespace".into()),
    }
}

pub(crate) fn parse_xml(text: &str) -> Result<Element> {
    let mut reader = NsReader::from_str(text);
    let mut stack = vec![Element::default()];
    loop {
        let event = reader.read_event()?;
        match event {
            Event::DocType(_) => return Err("DTD is prohibited in update feeds".into()),
            Event::Start(ref start) | Event::Empty(ref start) => {
                let (ns, name) = reader.resolver().resolve_element(start.name());
                let mut node = Element {
                    namespace: namespace(ns)?,
                    name: name.as_ref().to_owned(),
                    ..Element::default()
                };
                for attr in start.attributes() {
                    let attr = attr?;
                    let (ns, name) = reader.resolver().resolve_attribute(attr.key);
                    node.attrs.insert(
                        (namespace(ns)?, name.as_ref().to_owned()),
                        attr.normalized_value(XmlVersion::Implicit1_0)?.into_owned(),
                    );
                }
                if matches!(event, Event::Empty(_)) {
                    stack
                        .last_mut()
                        .ok_or("Invalid XML tree")?
                        .children
                        .push(node);
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                ensure(stack.len() > 1, "Unexpected XML end tag")?;
                let node = stack.pop().ok_or("Missing XML node")?;
                stack
                    .last_mut()
                    .ok_or("Missing XML parent")?
                    .children
                    .push(node);
            }
            Event::Text(value) => stack
                .last_mut()
                .ok_or("Missing XML node")?
                .text
                .push_str(&value.xml_content(XmlVersion::Implicit1_0)),
            Event::CData(value) => stack
                .last_mut()
                .ok_or("Missing XML node")?
                .text
                .push_str(&value.xml_content(XmlVersion::Implicit1_0)),
            Event::GeneralRef(value) => {
                let escaped = format!("&{};", value.as_ref());
                stack
                    .last_mut()
                    .ok_or("Missing XML node")?
                    .text
                    .push_str(&quick_xml::escape::unescape(&escaped)?);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ensure(
        stack.len() == 1 && stack[0].children.len() == 1 && stack[0].text.trim().is_empty(),
        "Expected one complete XML document",
    )?;
    Ok(stack
        .pop()
        .ok_or("Missing XML document")?
        .children
        .remove(0))
}
