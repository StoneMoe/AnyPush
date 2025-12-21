//! Template engine for rendering message content using minijinja

use crate::message::Message;
use minijinja::{context, Environment};

/// Render a template string with message context
pub fn render(template: &str, message: &Message) -> String {
    let env = Environment::new();

    match env.render_str(
        template,
        context! {
            id => message.id,
            timestamp => message.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            timestamp_unix => message.timestamp.timestamp(),
            raw => message.get_field("raw").unwrap_or_default(),
            raw_base64 => message.get_field("raw_base64").unwrap_or_default(),
            text => message.get_field("text").unwrap_or_default(),
            // Expose all metadata as a flat namespace
            file => context! {
                name => message.metadata().get_str("file.name").unwrap_or_default(),
                path => message.metadata().get_str("file.path").unwrap_or_default(),
                size => message.metadata().get_str("file.size").unwrap_or_default(),
            },
            smtp => context! {
                from => message.metadata().get_str("smtp.from").unwrap_or_default(),
                to => message.metadata().get_str("smtp.to").unwrap_or_default(),
                subject => message.metadata().get_str("smtp.subject").unwrap_or_default(),
            },
            http => context! {
                method => message.metadata().get_str("http.method").unwrap_or_default(),
                path => message.metadata().get_str("http.path").unwrap_or_default(),
                content_type => message.metadata().get_str("http.content_type").unwrap_or_default(),
            },
            tcp => context! {
                peer => message.metadata().get_str("tcp.peer").unwrap_or_default(),
            },
        },
    ) {
        Ok(rendered) => rendered,
        Err(e) => {
            tracing::warn!("Template render error: {}", e);
            template.to_string()
        }
    }
}
