//! Small adapter around the parser for the fixed, verified publication templates.
use crate::{protocol, OfficialNewsError};
use std::collections::HashMap;
use tl::{Node, NodeHandle, VDom};

pub(crate) struct Document<'a> {
    dom: VDom<'a>,
    elements: Vec<NodeHandle>,
    parents: HashMap<NodeHandle, NodeHandle>,
}

#[derive(Clone, Copy)]
pub(crate) struct Element<'d, 'a> {
    document: &'d Document<'a>,
    handle: NodeHandle,
}

impl<'a> Document<'a> {
    pub fn parse(html: &'a str) -> Result<Self, OfficialNewsError> {
        let dom = tl::parse(html, tl::ParserOptions::default())
            .map_err(|_| protocol("source HTML cannot be parsed"))?;
        let elements: Vec<_> = dom
            .query_selector("*")
            .ok_or_else(|| protocol("node selector is invalid"))?
            .filter(|handle| handle.get(dom.parser()).and_then(Node::as_tag).is_some())
            .collect();
        let mut parents = HashMap::new();
        for handle in &elements {
            let tag = handle
                .get(dom.parser())
                .and_then(Node::as_tag)
                .ok_or_else(|| protocol("source element is missing"))?;
            for child in tag.children().top().iter() {
                if parents.insert(*child, *handle).is_some() {
                    return Err(protocol("source node has ambiguous parents"));
                }
            }
        }
        Ok(Self {
            dom,
            elements,
            parents,
        })
    }

    pub fn select(&self, selector: &str) -> Result<Vec<Element<'_, 'a>>, OfficialNewsError> {
        // The underlying library matches simple selectors only. Resolve the
        // fixed profile's child/descendant relationships against actual parents.
        let mut steps = Vec::new();
        let mut direct = false;
        for token in selector.split_whitespace() {
            if token == ">" {
                if steps.is_empty() || direct {
                    return Err(protocol("source selector is invalid"));
                }
                direct = true;
            } else {
                steps.push((
                    tl::parse_query_selector(token)
                        .ok_or_else(|| protocol("source selector is invalid"))?,
                    direct,
                ));
                direct = false;
            }
        }
        if steps.is_empty() || direct {
            return Err(protocol("source selector is invalid"));
        }
        let mut selected = Vec::new();
        for handle in &self.elements {
            if self.matches_path(*handle, &steps, steps.len() - 1, 0)? {
                selected.push(self.element(*handle)?);
            }
        }
        Ok(selected)
    }

    fn matches_path(
        &self,
        handle: NodeHandle,
        steps: &[(tl::queryselector::Selector<'_>, bool)],
        index: usize,
        depth: usize,
    ) -> Result<bool, OfficialNewsError> {
        if depth > 256 {
            return Err(protocol("source HTML nesting exceeds the template bound"));
        }
        let node = handle
            .get(self.dom.parser())
            .ok_or_else(|| protocol("source node is missing"))?;
        if !steps[index].0.matches(node) {
            return Ok(false);
        }
        if index == 0 {
            return Ok(true);
        }
        let mut parent = self.parents.get(&handle).copied();
        let mut traversed = depth;
        while let Some(value) = parent {
            traversed += 1;
            if self.matches_path(value, steps, index - 1, traversed)? {
                return Ok(true);
            }
            if steps[index].1 {
                break;
            }
            if traversed > 256 {
                return Err(protocol("source HTML nesting exceeds the template bound"));
            }
            parent = self.parents.get(&value).copied();
        }
        Ok(false)
    }

    fn element(&self, handle: NodeHandle) -> Result<Element<'_, 'a>, OfficialNewsError> {
        if handle
            .get(self.dom.parser())
            .and_then(Node::as_tag)
            .is_none()
        {
            return Err(protocol("source element is missing"));
        }
        Ok(Element {
            document: self,
            handle,
        })
    }
}

impl<'d, 'a> Element<'d, 'a> {
    pub fn attr(self, name: &str) -> Option<String> {
        self.handle
            .get(self.document.dom.parser())?
            .as_tag()?
            .attributes()
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, value)| value)
            .map(|value| html_escape::decode_html_entities(&value).into_owned())
    }

    pub fn select(self, selector: &str) -> Result<Vec<Self>, OfficialNewsError> {
        let mut selected = Vec::new();
        for element in self.document.select(selector)? {
            let mut parent = self.document.parents.get(&element.handle).copied();
            for _ in 0..=256 {
                let Some(handle) = parent else {
                    break;
                };
                if handle == self.handle {
                    selected.push(element);
                    break;
                }
                parent = self.document.parents.get(&handle).copied();
            }
            if parent.is_some()
                && !selected
                    .last()
                    .is_some_and(|item| item.handle == element.handle)
            {
                return Err(protocol("source HTML nesting exceeds the template bound"));
            }
        }
        Ok(selected)
    }

    pub fn parent(self) -> Result<Self, OfficialNewsError> {
        self.document.element(
            *self
                .document
                .parents
                .get(&self.handle)
                .ok_or_else(|| protocol("listing date container is missing"))?,
        )
    }

    pub fn text(self) -> Result<String, OfficialNewsError> {
        // Iterative traversal avoids stack growth for deeply nested source HTML.
        let mut stack = vec![(self.handle, 0usize)];
        let mut parts = Vec::new();
        while let Some((handle, depth)) = stack.pop() {
            if depth > 256 {
                return Err(protocol("source HTML nesting exceeds the template bound"));
            }
            match handle.get(self.document.dom.parser()) {
                Some(Node::Tag(tag)) => {
                    if ["script", "style", "noscript"]
                        .iter()
                        .any(|name| tag.name().as_utf8_str().eq_ignore_ascii_case(name))
                    {
                        continue;
                    }
                    let children = tag.children().top().to_vec();
                    stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
                }
                Some(Node::Raw(text)) => {
                    parts.push(html_escape::decode_html_entities(&text.as_utf8_str()).into_owned())
                }
                Some(Node::Comment(_)) => {}
                None => return Err(protocol("source text node is missing")),
            }
        }
        Ok(parts
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "))
    }
}
