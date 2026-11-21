use serde_json::json;

pub struct Email<'a> {
    pub to: &'a str,
    pub subject: &'a str,
    pub text: &'a str,
    pub html: &'a str,
}

/// Every email the server sends goes through `Mailer::send`. Adding a provider is one more variant
/// and one more arm; `MAIL_PROVIDER` picks it.
pub enum Mailer {
    Brevo { key: String, from: String, from_name: String },
    Log,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

impl Mailer {
    pub fn from_env() -> Self {
        let provider = env("MAIL_PROVIDER").unwrap_or_else(|| if env("BREVO_API_KEY").is_some() { "brevo".into() } else { "log".into() });
        let from_name = env("MAIL_FROM_NAME").unwrap_or_else(|| "SkibiDink".into());
        match provider.as_str() {
            "brevo" => match (env("BREVO_API_KEY"), env("MAIL_FROM")) {
                (Some(key), Some(from)) => Mailer::Brevo { key, from, from_name },
                _ => {
                    eprintln!("[mail] MAIL_PROVIDER=brevo needs BREVO_API_KEY and MAIL_FROM: emails go to the log");
                    Mailer::Log
                }
            },
            "log" => Mailer::Log,
            other => {
                eprintln!("[mail] unknown MAIL_PROVIDER {other:?}: emails go to the log");
                Mailer::Log
            }
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Mailer::Brevo { .. } => "brevo",
            Mailer::Log => "log",
        }
    }

    pub async fn send(&self, http: &reqwest::Client, email: Email<'_>) -> Result<(), String> {
        match self {
            Mailer::Brevo { key, from, from_name } => {
                let body = json!({
                    "sender": { "name": from_name, "email": from },
                    "to": [{ "email": email.to }],
                    "subject": email.subject,
                    "htmlContent": email.html,
                    "textContent": email.text,
                });
                let res = http.post("https://api.brevo.com/v3/smtp/email").header("api-key", key).json(&body).send().await.map_err(|e| e.to_string())?;
                if res.status().is_success() {
                    Ok(())
                } else {
                    let status = res.status();
                    Err(format!("Brevo {status}: {}", res.text().await.unwrap_or_default()))
                }
            }
            Mailer::Log => {
                println!("[mail] to {} | {}\n{}", email.to, email.subject, email.text);
                Ok(())
            }
        }
    }
}
