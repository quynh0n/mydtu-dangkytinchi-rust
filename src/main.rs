use clap::{App, Arg};
use serde::Deserialize;
use std::{collections::HashMap, fs, sync::Arc};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio;

use base64::encode as base64_encode;
use openssl::symm::{encrypt as encrypt_aes, Cipher};

use reqwest;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Deserialize)]
struct Account {
    session_id: String,
    class_codes: Vec<String>,
    student_id: String,
    curriculum_id: String,
    captcha: String,
}

#[derive(Debug, Deserialize)]
struct Accounts {
    acc: Vec<Account>,
}

#[derive(Debug, Deserialize)]
struct MyDtuResponse {
    d: String,
}

fn read_accounts_from_file(filename: &str) -> Result<Accounts> {
    let contents = fs::read_to_string(filename)?;
    Ok(toml::from_str(&contents)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("MyDTU-DKTC")
    .version("0.1.0")
    .author("Son Tran <trankyson@dtu.edu.vn>")
    .about("Dang ki tin chi")
    .arg(
        Arg::with_name("file")
            .short("f")
            .long("file")
            .takes_value(true)
            .help("The file contains your account(s).")
            .default_value("accounts.toml")
    )
    .arg(
        Arg::with_name("year")
            .short("y")
            .long("year")
            .takes_value(true)
            .required(true)
            .help("Year of academic")
    )
    .arg(
        Arg::with_name("semester")
            .short("s")
            .long("semester")
            .takes_value(true)
            .required(true)
            .help("Semester of year")
    )
    .arg(
        Arg::with_name("sleep")
            .long("sleep")
            .takes_value(true)
            .help("The second that the program will wait between each request.")
            .default_value("5")
    )
    .get_matches();

    let endpoint = "https://mydtu.duytan.edu.vn/Modules/regonline/ajax/RegistrationProcessing.aspx/AddClassProcessing";
    let filename = matches.value_of("file").unwrap();
    let year = matches.value_of("year").unwrap().to_string();
    let semester = matches.value_of("semester").unwrap().to_string();
    let accounts = read_accounts_from_file(filename)?;
    
    let mut state: HashMap<String, String> = HashMap::new();
    state.insert("year".to_string(), year);
    state.insert("semester".to_string(), semester);

    let client = reqwest::Client::new();
    let state = Arc::new(Mutex::new(state));

    let mut tasks: Vec<JoinHandle<()>> = Vec::new();
    for acc in accounts.acc.into_iter() {
        let client = client.clone();
        let state = state.clone();
        let acc = Arc::new(acc);
        let result = tokio::spawn(async move {
            let class_codes = acc.class_codes.clone();
            
            for class_code in class_codes.into_iter() {
                let acc = acc.clone();
                let client = client.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    let state = state.lock().await;
                    
                    let year = state.get("year").unwrap().to_owned();
                    let semester = state.get("semester").unwrap().to_owned();
                    
                    let student_id = acc.student_id.clone();
                    let curriculum_id = acc.curriculum_id.clone();
                    let captcha = acc.captcha.clone();

                    let params: String = vec![class_code, year, semester, student_id, curriculum_id, captcha].join(",");

                    let cipher = Cipher::aes_256_cbc();
                    let key = b"AMINHAKEYTEM32NYTES1234567891234";
                    let iv = b"7061737323313233"; // pass#123
                    
                    let ciphertext = encrypt_aes(cipher, key, Some(iv), params.as_bytes()).unwrap();
                    let ciphertext = base64_encode(ciphertext);
                    let mut json_map = HashMap::new();
                    json_map.insert("encryptedPara", ciphertext);
                    
                    let session_id = acc.session_id.clone();
                    
                    let response = client.post(endpoint)
                        .header(reqwest::header::COOKIE, format!("ASP.NET_SessionId={}", session_id))
                        .json(&json_map)
                        .send().await
                        .unwrap()
                        .json::<MyDtuResponse>().await
                        .unwrap();
                    println!("{}", response.d)
                }).await
                .unwrap()
            }
        });
        tasks.push(result);
    };

    for task in tasks {
        task.await?;
    }

    Ok(())
}
