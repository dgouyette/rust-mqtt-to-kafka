use std::{env, process, thread};
use std::collections::HashMap;
use std::time::Duration;

use ctrlc;
use kafka::error::Error as KafkaError;
use kafka::producer::{Producer, Record, RequiredAcks};
use paho_mqtt as mqtt;

fn try_reconnect(cli: &mqtt::Client) -> bool {
    println!("Connection lost. Waiting to retry connection");
    for _ in 0..12 {
        thread::sleep(Duration::from_millis(5000));
        if cli.reconnect().is_ok() {
            println!("Successfully reconnected");
            return true;
        }
    }
    println!("Unable to reconnect after several attempts.");
    false
}

fn produce_message(
    data: &[u8],
    topic: &str,
    brokers: Vec<String>,
) -> Result<(), KafkaError> {
    //println!("About to publish a message at {:?} to: {}", brokers, topic);

    // ~ create a producer. this is a relatively costly operation, so
    // you'll do this typically once in your application and re-use
    // the instance many times.
    let mut producer = Producer::from_hosts(brokers)
        // ~ give the brokers one second time to ack the message
        .with_ack_timeout(Duration::from_secs(1))
        // ~ require only one broker to ack the message
        .with_required_acks(RequiredAcks::One)
        // ~ build the producer with the above settings
        .create()?;


    producer.send(&Record::from_value(topic, data))?;

    Ok(())
}

fn main() {
    let broker = "192.168.1.29:9092";

    let host = env::args()
        .nth(1)
        .unwrap_or_else(|| "mqtt://192.168.1.29:1883".to_string());

    println!("{host}");
    let create_opts = mqtt::CreateOptionsBuilder::new()
        .server_uri(host)
        .client_id("rust_sync_consumer")
        .finalize();

    let cli = mqtt::Client::new(create_opts).unwrap_or_else(|e| {
        println!("Error creating the client: {:?}", e);
        process::exit(1);
    });

    let rx = cli.start_consuming();

    let lwt = mqtt::MessageBuilder::new()
        .topic("Production")
        .payload("Sync consumer lost connection")
        .finalize();

    let conn_opts = mqtt::ConnectOptionsBuilder::new()
        .keep_alive_interval(Duration::from_secs(20))
        .clean_session(false)
        .will_message(lwt)
        .finalize();

    println!("Connecting to the MQTT broker...");
    let subscriptions = ["Production/#", "Consommation/#"];


    let qos = [1, 1];

    match cli.connect(conn_opts) {
        Ok(rsp) => {
            if let Some(conn_rsp) = rsp.connect_response() {
                println!(
                    "Connected to: '{}' with MQTT version {}",
                    conn_rsp.server_uri, conn_rsp.mqtt_version
                );

                println!("Subscribing to topics with requested QoS: {:?}...", qos);

                cli.subscribe_many(&subscriptions, &qos)
                    .and_then(|rsp| {
                        rsp.subscribe_many_response()
                            .ok_or(mqtt::Error::General("Bad response"))
                    })
                    .and_then(|vqos| {
                        println!("QoS granted: {:?}", vqos);
                        Ok(())
                    })
                    .unwrap_or_else(|err| {
                        println!("Error subscribing to topics: {:?}", err);
                        cli.disconnect(None).unwrap();
                        process::exit(1);
                    });
            }
        }
        Err(e) => {
            println!("Error connecting to the broker: {:?}", e);
            process::exit(1);
        }
    }

    let ctrlc_cli = cli.clone();
    ctrlc::set_handler(move || {
        ctrlc_cli.stop_consuming();
    })
        .expect("Error setting Ctrl-C handler");


    let mapping = HashMap::from([
        ("Production/metrics/W", "production_w"),
        ("Consommation/metrics/ActivatePower", "consommation_w"),
        ("Consommation/metrics/ImportEnergy", "import_w"),
        ("Consommation/metrics/ExportEnergy", "export_w"),
    ]);

    println!("\nWaiting for messages on topics {:?}...", subscriptions);
    for msg in rx.iter() {
        if let Some(msg) = msg {
            if let Some(topic) = mapping.get(msg.topic()) {
                if let Ok(_) = produce_message(msg.payload_str().as_bytes(), topic, vec![broker.to_owned()]) {
                    println!("{topic}:{}", msg.payload_str());
                }
            }
        } else if cli.is_connected() || !try_reconnect(&cli) {
            break;
        }
    }


    if cli.is_connected() {
        println!("\nDisconnecting...");
        cli.disconnect(None).unwrap();
    }
}
