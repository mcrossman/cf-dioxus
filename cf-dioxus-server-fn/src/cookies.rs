/// Cookie management module for HTTP cookie handling in server functions
/// This module provides utilities for reading and setting HTTP cookies
/// in the Cloudflare Workers environment with cf-dioxus server functions.

use std::collections::HashMap;

/// Extension data type for cookies passed to server functions
#[derive(Debug, Clone)]
pub struct RequestCookies {
    /// Map of cookie name to value
    cookies: HashMap<String, String>,
}

impl RequestCookies {
    /// Create a new RequestCookies from a Cookie header value
    pub fn from_cookie_header(cookie_header: Option<&str>) -> Self {
        let mut cookies = HashMap::new();

        if let Some(header) = cookie_header {
            for cookie_str in header.split(';') {
                let cookie_str = cookie_str.trim();
                if let Some((name, value)) = cookie_str.split_once('=') {
                    cookies.insert(name.trim().to_string(), value.trim().to_string());
                }
            }
        }

        Self { cookies }
    }

    /// Get a specific cookie value by name
    pub fn get(&self, name: &str) -> Option<&str> {
        self.cookies.get(name).map(|s| s.as_str())
    }
}

impl Default for RequestCookies {
    fn default() -> Self {
        Self {
            cookies: HashMap::new(),
        }
    }
}

/// Response cookie to be set
#[derive(Debug, Clone)]
pub struct ResponseCookie {
    /// Cookie name
    pub name: String,
    /// Cookie value
    pub value: String,
    /// Whether to set an expired date to clear the cookie
    pub clear: bool,
}

impl ResponseCookie {
    /// Create a new cookie to set
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            clear: false,
        }
    }

    /// Create a cookie to clear (sets expired date)
    pub fn clear(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: String::new(),
            clear: true,
        }
    }

    /// Format the cookie as a Set-Cookie header value
    pub fn to_set_cookie_header(&self) -> String {
        if self.clear {
            format!("{}=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/; Secure; HttpOnly; SameSite=Lax",
                self.name)
        } else {
            format!("{}={}; Path=/; Secure; HttpOnly; SameSite=Lax",
                self.name, self.value)
        }
    }
}

/// Extension type for response cookies - allows server functions to queue cookies to set
#[derive(Debug, Default, Clone)]
pub struct ResponseCookies {
    cookies: Vec<ResponseCookie>,
}

impl ResponseCookies {
    /// Add a cookie to set
    pub fn add(&mut self, cookie: ResponseCookie) {
        self.cookies.push(cookie);
    }

    /// Get all cookies to set
    pub fn into_headers(self) -> Vec<String> {
        self.cookies.into_iter()
            .map(|c| c.to_set_cookie_header())
            .collect()
    }
}
