use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const FIGMA_API_BASE: &str = "https://api.figma.com/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FigmaFile {
    pub key: String,
    pub name: String,
    pub last_modified: String,
    pub thumbnail_url: Option<String>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FigmaNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub children: Option<Vec<FigmaNode>>,
    #[serde(flatten)]
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FigmaFileData {
    pub document: FigmaNode,
    pub components: HashMap<String, serde_json::Value>,
    pub styles: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignToken {
    pub name: String,
    pub token_type: String,
    pub value: serde_json::Value,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FigmaComment {
    pub id: String,
    pub message: String,
    pub created_at: String,
    pub user: FigmaUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FigmaUser {
    pub id: String,
    pub handle: String,
    pub img_url: Option<String>,
}

pub struct FigmaClient {
    api_key: String,
    client: reqwest::Client,
}

impl FigmaClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    fn get_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "X-Figma-Token",
            reqwest::header::HeaderValue::from_str(&self.api_key).unwrap(),
        );
        headers
    }

    pub async fn get_file(&self, file_key: &str) -> Result<FigmaFileData, String> {
        let url = format!("{}/files/{}?geometry=paths", FIGMA_API_BASE, file_key);
        
        let response = self
            .client
            .get(&url)
            .headers(self.get_headers())
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("API error: {} - {}", status, text));
        }

        let data: FigmaFileData = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(data)
    }

    pub async fn get_file_info(&self, file_key: &str) -> Result<FigmaFile, String> {
        let url = format!("{}/files/{}", FIGMA_API_BASE, file_key);
        
        let response = self
            .client
            .get(&url)
            .headers(self.get_headers())
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("API error: {} - {}", status, text));
        }

        let data: FigmaFile = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(data)
    }

    pub async fn get_images(&self, file_key: &str, node_ids: &[String]) -> Result<HashMap<String, String>, String> {
        let ids = node_ids.join(",");
        let url = format!(
            "{}/images/{}?ids={}&format=png&scale=2",
            FIGMA_API_BASE, file_key, ids
        );
        
        let response = self
            .client
            .get(&url)
            .headers(self.get_headers())
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("API error: {} - {}", status, text));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        let images = data
            .get("images")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| {
                        v.as_str().map(|url| (k.clone(), url.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(images)
    }

    pub async fn get_comments(&self, file_key: &str) -> Result<Vec<FigmaComment>, String> {
        let url = format!("{}/files/{}/comments", FIGMA_API_BASE, file_key);
        
        let response = self
            .client
            .get(&url)
            .headers(self.get_headers())
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("API error: {} - {}", status, text));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        let comments: Vec<FigmaComment> = data
            .get("comments")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(comments)
    }
}

pub fn extract_design_tokens(node: &FigmaNode) -> Vec<DesignToken> {
    let mut tokens = Vec::new();
    
    if let Some(fills) = node.properties.get("fills") {
        if let Some(fill_array) = fills.as_array() {
            for (i, fill) in fill_array.iter().enumerate() {
                if let Some(color) = fill.get("color") {
                    let token = DesignToken {
                        name: format!("{}/{}/fill-{}", node.name, node.node_type, i),
                        token_type: "color".to_string(),
                        value: color.clone(),
                        description: None,
                    };
                    tokens.push(token);
                }
            }
        }
    }
    
    if let Some(style) = node.properties.get("style") {
        if let Some(font_family) = style.get("fontFamily") {
            tokens.push(DesignToken {
                name: format!("{}/font-family", node.name),
                token_type: "typography".to_string(),
                value: font_family.clone(),
                description: None,
            });
        }
        
        if let Some(font_size) = style.get("fontSize") {
            tokens.push(DesignToken {
                name: format!("{}/font-size", node.name),
                token_type: "typography".to_string(),
                value: font_size.clone(),
                description: None,
            });
        }
    }
    
    if let Some(children) = &node.children {
        for child in children {
            tokens.extend(extract_design_tokens(child));
        }
    }
    
    tokens
}

pub fn extract_css_from_node(node: &FigmaNode) -> String {
    let mut css = String::new();
    
    let selector = node.name.to_lowercase().replace(" ", "-").replace("/", "-");
    css.push_str(&format!(".{} {{\n", selector));
    
    if let Some(absolute_bounding_box) = node.properties.get("absoluteBoundingBox") {
        if let Some(width) = absolute_bounding_box.get("width") {
            if let Some(w) = width.as_f64() {
                css.push_str(&format!("  width: {}px;\n", w));
            }
        }
        if let Some(height) = absolute_bounding_box.get("height") {
            if let Some(h) = height.as_f64() {
                css.push_str(&format!("  height: {}px;\n", h));
            }
        }
    }
    
    if let Some(fills) = node.properties.get("fills") {
        if let Some(fill_array) = fills.as_array() {
            if let Some(first_fill) = fill_array.first() {
                if let Some(color) = first_fill.get("color") {
                    let r = color.get("r").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let g = color.get("g").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let b = color.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let a = color.get("a").and_then(|v| v.as_f64()).unwrap_or(1.0);
                    
                    if a < 1.0 {
                        css.push_str(&format!(
                            "  background-color: rgba({}, {}, {}, {:.2});\n",
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (b * 255.0) as u8,
                            a
                        ));
                    } else {
                        css.push_str(&format!(
                            "  background-color: rgb({}, {}, {});\n",
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (b * 255.0) as u8
                        ));
                    }
                }
            }
        }
    }
    
    if let Some(style) = node.properties.get("style") {
        if let Some(font_family) = style.get("fontFamily") {
            if let Some(family) = font_family.as_str() {
                css.push_str(&format!("  font-family: '{}';\n", family));
            }
        }
        if let Some(font_size) = style.get("fontSize") {
            if let Some(size) = font_size.as_f64() {
                css.push_str(&format!("  font-size: {}px;\n", size));
            }
        }
        if let Some(font_weight) = style.get("fontWeight") {
            if let Some(weight) = font_weight.as_f64() {
                css.push_str(&format!("  font-weight: {};\n", weight as i32));
            }
        }
    }
    
    css.push_str("}\n\n");
    
    if let Some(children) = &node.children {
        for child in children {
            css.push_str(&extract_css_from_node(child));
        }
    }
    
    css
}

pub fn find_nodes_by_type(node: &FigmaNode, node_type: &str) -> Vec<FigmaNode> {
    let mut results = Vec::new();
    
    if node.node_type == node_type {
        results.push(node.clone());
    }
    
    if let Some(children) = &node.children {
        for child in children {
            results.extend(find_nodes_by_type(child, node_type));
        }
    }
    
    results
}

pub fn find_nodes_by_name(node: &FigmaNode, name_pattern: &str) -> Vec<FigmaNode> {
    let mut results = Vec::new();
    
    if node.name.to_lowercase().contains(&name_pattern.to_lowercase()) {
        results.push(node.clone());
    }
    
    if let Some(children) = &node.children {
        for child in children {
            results.extend(find_nodes_by_name(child, name_pattern));
        }
    }
    
    results
}
