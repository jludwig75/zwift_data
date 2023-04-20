use std::vec;

use anyhow::{anyhow, Result};
use async_recursion::async_recursion;
use html_parser::{Dom, Element, Node};

pub async fn find<'a>(element: &'a Element, selectors_string: &str) -> Result<Vec<&'a Element>> {
    find_elements(element, &parse_selector_string(selectors_string)?).await
}

pub async fn select<'a>(dom: &'a Dom, selectors_string: &str) -> Result<Vec<&'a Element>> {
    let selectors = parse_selector_string(selectors_string)?;
    let mut elements = Vec::new();
    for child in &dom.children {
        if let Node::Element(element) = child {
            elements.append(&mut find_elements(element, &selectors).await?);
        }
    }

    Ok(elements)
}

#[derive(Debug)]
enum BasicSelector {
    All,
    Id(String),
    Class(String),
    Element(String),
}

impl Clone for BasicSelector {
    fn clone(&self) -> Self {
        match self {
            BasicSelector::All => BasicSelector::All,
            BasicSelector::Id(str) => BasicSelector::Id(str.clone()),
            BasicSelector::Class(str) => BasicSelector::Class(str.clone()),
            BasicSelector::Element(str) => BasicSelector::Element(str.clone()),
        }
    }
}

#[derive(Debug)]
enum Selector {
    Basic(BasicSelector),
    Hierarchical(Vec<BasicSelector>),
    Intersection(Vec<BasicSelector>),
}

fn parse_individual_selector_string(selector_string: &str) -> Result<BasicSelector> {
    // Must not contain white space
    assert!(
        !selector_string.contains(' ')
            && !selector_string.contains('\t')
            && !selector_string.contains('\n')
    );
    // Must also not contain . after first charactet
    assert!(!selector_string[1..].contains('.'));
    if selector_string == "*" {
        // No need to consider other selectors if * is specified
        Ok(BasicSelector::All)
    } else if let Some(class) = selector_string.strip_prefix('.') {
        if class.is_empty() {
            Err(anyhow!(
                "Invalid query string \"{selector_string}\": Single '.' not valid"
            ))
        } else {
            Ok(BasicSelector::Class(class.into()))
        }
    } else if let Some(id) = selector_string.strip_prefix('#') {
        if id.is_empty() {
            Err(anyhow!(
                "Invalid query string \"{selector_string}\": Single '#' not valid"
            ))
        } else if id[1..].contains('#') || id[1..].contains('#') {
            Err(anyhow!("Invalid query string \"{selector_string}\": selectors can only contain one element ID"))
        } else {
            Ok(BasicSelector::Id(id.into()))
        }
    } else {
        Ok(BasicSelector::Element(selector_string.into()))
    }
}

fn parse_selector_string(selector_string: &str) -> Result<Vec<Selector>> {
    let mut selectors = Vec::new();
    for item in selector_string.split(',') {
        let selector_strings: Vec<_> = item.split_ascii_whitespace().collect();
        match selector_strings.len() {
            0 => return Err(anyhow!("Invalid query string: {}", selector_string)),
            1 => selectors.push(Selector::Basic(parse_individual_selector_string(
                selector_strings[0],
            )?)),
            _ => {
                // White space seperated selectors are hierarchical.
                let mut hierarchical_selectors = Vec::new();
                for selector_string in selector_strings {
                    hierarchical_selectors.push(parse_individual_selector_string(selector_string)?)
                }
                assert!(hierarchical_selectors.len() > 1);
                selectors.push(Selector::Hierarchical(hierarchical_selectors));
            }
        }
    }

    println!("Selectors: {:#?}", selectors);

    Ok(selectors)
}

fn element_matches_basic_selector(element: &Element, basic_selector: &BasicSelector) -> bool {
    match basic_selector {
        BasicSelector::All => true,
        BasicSelector::Id(id) => {
            if let Some(element_id) = &element.id {
                *id == *element_id
            } else {
                false
            }
        }
        BasicSelector::Class(class) => element.classes.contains(class),
        BasicSelector::Element(tag) => *tag == element.name,
    }
}

#[async_recursion]
async fn find_elements_for_selector<'a>(
    element: &'a Element,
    selector: &Selector,
) -> Result<Vec<&'a Element>> {
    match selector {
        Selector::Basic(basic_selector) => {
            if element_matches_basic_selector(element, basic_selector) {
                Ok(vec![element])
            } else {
                Ok(vec![])
            }
        }
        Selector::Intersection(basic_selectors) => {
            let mut matches = true;
            for basic_selector in basic_selectors {
                if !element_matches_basic_selector(element, basic_selector) {
                    matches = false;
                    break;
                }
            }
            if matches {
                Ok(vec![element])
            } else {
                Ok(vec![])
            }
        }
        Selector::Hierarchical(basic_selectors) => {
            println!(
                "Looking for hierarchical selector {:?} at element {:?}",
                selector, element
            );
            if element_matches_basic_selector(element, &basic_selectors[0]) {
                println!("First hierarchical selector matches");
                if basic_selectors.len() == 1 {
                    Ok(vec![element])
                } else {
                    let basic_selector_list = basic_selectors.clone()[1..].to_vec();
                    let mut elements = Vec::new();
                    for child in &element.children {
                        if let Node::Element(element) = child {
                            elements.append(
                                &mut find_elements_for_selector(
                                    element,
                                    &Selector::Hierarchical(basic_selector_list.clone()),
                                )
                                .await?,
                            );
                        }
                    }

                    Ok(elements)
                }
            } else {
                let mut elements = Vec::new();
                for child in &element.children {
                    if let Node::Element(element) = child {
                        elements.append(&mut find_elements_for_selector(element, selector).await?);
                    }
                }

                Ok(elements)
            }
        }
    }
}

#[async_recursion]
async fn find_elements<'a>(
    element: &'a Element,
    selectors: &Vec<Selector>,
) -> Result<Vec<&'a Element>> {
    let mut elements = Vec::new();

    for selector in selectors {
        let matching_elements = find_elements_for_selector(element, selector).await?;
        for matching_element in matching_elements {
            if !elements.contains(&matching_element) {
                elements.push(matching_element);
            }
        }
    }

    for child in &element.children {
        if let Node::Element(element) = child {
            elements.append(&mut find_elements(element, selectors).await?);
        }
    }

    Ok(elements)
}

#[cfg(test)]
mod test {
    use html_parser::{Dom, Node};

    use super::{find, select};

    static TEST_HTML: &str = r#"<div id="myDiv">
  <h1 class="title">Title</h1>
  <p class="intro">Introduction</p>
    <ul>
      <li class="item">Item 1</li>
      <li class="item">Item 2</li>
    </ul>
</div>"#;

    /*
        Selector String     Expected Result
        ---------------     ---------------
    //  "#myDiv"            <div id="myDiv">...</div>
    //  ".title"            <h1 class="title">...</h1>
    //  "h1"                <h1 class="title">...</h1>
    //  "ul"                <ul>...</ul>
    //  "li"                <li class="item">Item 1</li>, <li class="item">Item 2</li>
    //  "#myDiv,h1"         <h1 class="title">...</h1>
    //  "h1,p"             <h1 class="title">...</h1>, <p class="intro">...</p>
    //  "ul,li"            <ul>...</ul>, <li class="item">Item 1</li>, <li class="item">Item 2</li>
    //  "#myDiv .item"      <li class="item">Item 1</li>, <li class="item">Item 2</li>
    //  "#myDiv h1"         <h1 class="title">...</h1>

        TODO: Add support for this. Need to find out rules.
    //  "li.item"           <li class="item">Item 1</li>, <li class="item">Item 2</li>
    //  "#myDiv,li.item"    <li class="item">Item 1</li>, <li class="item">Item 2</li>
    */

    #[tokio::test]
    async fn test_select() {
        let dom = Dom::parse(TEST_HTML).unwrap();

        // *
        let elements = select(&dom, "*").await.unwrap();
        assert_eq!(elements.len(), 6);

        //  "#myDiv"            <div id="myDiv">...</div>
        let elements = select(&dom, "#myDiv").await.unwrap();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].name, "div");
        assert_eq!(elements[0].id, Some("myDiv".to_string()));

        //  ".title"            <h1 class="title">...</h1>
        let elements = select(&dom, ".title").await.unwrap();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].name, "h1");
        assert!(elements[0].classes.contains(&"title".to_string()));

        //  "h1"                <h1 class="title">...</h1>
        let elements = select(&dom, "h1").await.unwrap();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].name, "h1");
        assert!(elements[0].classes.contains(&"title".to_string()));

        //  "ul"                <ul>...</ul>
        let elements = select(&dom, "ul").await.unwrap();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].name, "ul");

        //  "li"                <li class="item">Item 1</li>, <li class="item">Item 2</li>
        let elements = select(&dom, "li").await.unwrap();
        assert_eq!(elements.len(), 2);

        assert_eq!(elements[0].name, "li");
        assert!(elements[0].classes.contains(&"item".to_string()));
        assert_eq!(elements[0].children[0], Node::Text("Item 1".to_string()));

        assert_eq!(elements[1].name, "li");
        assert!(elements[1].classes.contains(&"item".to_string()));
        assert_eq!(elements[1].children[0], Node::Text("Item 2".to_string()));

        //  "#myDiv,h1"         <h1 class="title">...</h1>
        let elements = select(&dom, "#myDiv,h1").await.unwrap();
        assert_eq!(elements.len(), 2);

        assert_eq!(elements[0].name, "div");
        assert_eq!(elements[0].id, Some("myDiv".to_string()));

        assert_eq!(elements[1].name, "h1");
        assert!(elements[1].classes.contains(&"title".to_string()));

        //  "h1,p"             <h1 class="title">...</h1>, <p class="intro">...</p>
        let elements = select(&dom, "h1,p").await.unwrap();
        assert_eq!(elements.len(), 2);

        assert_eq!(elements[0].name, "h1");
        assert!(elements[0].classes.contains(&"title".to_string()));
        assert_eq!(elements[0].children[0], Node::Text("Title".to_string()));

        assert_eq!(elements[1].name, "p");
        assert!(elements[1].classes.contains(&"intro".to_string()));
        assert_eq!(
            elements[1].children[0],
            Node::Text("Introduction".to_string())
        );

        //  "ul,li"            <ul>...</ul>, <li class="item">Item 1</li>, <li class="item">Item 2</li>
        let elements = select(&dom, "ul,li").await.unwrap();
        assert_eq!(elements.len(), 3);

        assert_eq!(elements[0].name, "ul");

        assert_eq!(elements[1].name, "li");
        assert!(elements[1].classes.contains(&"item".to_string()));
        assert_eq!(elements[1].children[0], Node::Text("Item 1".to_string()));

        assert_eq!(elements[2].name, "li");
        assert!(elements[2].classes.contains(&"item".to_string()));
        assert_eq!(elements[2].children[0], Node::Text("Item 2".to_string()));

        //  "#myDiv .item"      <li class="item">Item 1</li>, <li class="item">Item 2</li>
        let elements = select(&dom, "#myDiv .item").await.unwrap();
        assert_eq!(elements.len(), 2);

        assert_eq!(elements[0].name, "li");
        assert!(elements[0].classes.contains(&"item".to_string()));
        assert_eq!(elements[0].children[0], Node::Text("Item 1".to_string()));

        assert_eq!(elements[1].name, "li");
        assert!(elements[1].classes.contains(&"item".to_string()));
        assert_eq!(elements[1].children[0], Node::Text("Item 2".to_string()));

        //  "#myDiv h1"         <h1 class="title">...</h1>
    }

    #[tokio::test]
    async fn test_find() {
        let dom = Dom::parse(TEST_HTML).unwrap();

        let dom_elements = select(&dom, "ul").await.unwrap();
        assert_eq!(dom_elements.len(), 1);
        assert_eq!(dom_elements[0].name, "ul");

        let child_element = dom_elements[0];

        // *
        let elements = find(child_element, "*").await.unwrap();
        assert_eq!(elements.len(), 3);

        // p
        let elements = find(child_element, "p").await.unwrap();
        assert!(elements.is_empty());

        // li
        let elements = find(child_element, "li").await.unwrap();
        assert_eq!(elements.len(), 2);

        assert_eq!(elements[0].name, "li");
        assert!(elements[0].classes.contains(&"item".to_string()));
        assert_eq!(elements[0].children[0], Node::Text("Item 1".to_string()));

        assert_eq!(elements[1].name, "li");
        assert!(elements[1].classes.contains(&"item".to_string()));
        assert_eq!(elements[1].children[0], Node::Text("Item 2".to_string()));

        // .item
        let elements = find(child_element, ".item").await.unwrap();
        assert_eq!(elements.len(), 2);

        assert_eq!(elements[0].name, "li");
        assert!(elements[0].classes.contains(&"item".to_string()));
        assert_eq!(elements[0].children[0], Node::Text("Item 1".to_string()));

        assert_eq!(elements[1].name, "li");
        assert!(elements[1].classes.contains(&"item".to_string()));
        assert_eq!(elements[1].children[0], Node::Text("Item 2".to_string()));

        // li,.item
        let elements = find(child_element, "li,.item").await.unwrap();
        assert_eq!(elements.len(), 2);

        assert_eq!(elements[0].name, "li");
        assert!(elements[0].classes.contains(&"item".to_string()));
        assert_eq!(elements[0].children[0], Node::Text("Item 1".to_string()));

        assert_eq!(elements[1].name, "li");
        assert!(elements[1].classes.contains(&"item".to_string()));
        assert_eq!(elements[1].children[0], Node::Text("Item 2".to_string()));
    }
}
