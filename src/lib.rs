use godot::prelude::*;
use convex::base_client::{
    BaseConvexClient,
    FunctionResult
};
use convex_sync_types::{
    UdfPath,
    ServerMessage,
};
use convex::{
    SubscriberId,
    Value,
};
use std::str::FromStr;
use std::convert::TryFrom;
use std::collections::BTreeMap;
use serde_json::{Value as JsonValue};
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::error::TryRecvError;

struct ConvexGd;

#[gdextension]
unsafe impl ExtensionLibrary for ConvexGd {}


#[derive(GodotClass)]
#[class(no_init)]
pub struct ConvexClient {
    base_client: BaseConvexClient,
}

#[derive(GodotClass)]
#[class(no_init)]
pub struct Subscription {
    #[var]
    success: bool,
    subscriber_id: Option<SubscriberId>,
}

#[derive(GodotClass)]
#[class(no_init)]
pub struct ResultReceiver {
    #[var]
    success: bool,
    receiver: Option<Receiver<FunctionResult>>,
}

#[godot_api]
impl ResultReceiver {
    #[func]
    pub fn get_result(&mut self) -> Dictionary {
        // Temporarily take the receiver to get mutable access
        if let Some(mut receiver) = self.receiver.take() {
            match receiver.try_recv() {
                Ok(result) => {
                    // If successful, put the receiver back
                    self.receiver = Some(receiver);
                    convert_function_result_to_dictionary(Some(result))
                },
                Err(TryRecvError::Empty) => {
                    // If the receiver is empty, put it back and return an empty Dictionary
                    self.receiver = Some(receiver);
                    Dictionary::new()
                },
                Err(TryRecvError::Closed) => {
                    // If the receiver is closed, it's already taken out, so just return an empty Dictionary
                    Dictionary::new()
                },
            }
        } else {
            // If there was no receiver, just return an empty Dictionary
            Dictionary::new()
        }
    }
}

#[godot_api]
impl ConvexClient {
    // A godot object with access to the Convex state machine
    // you don't need to wrap with the Gd pointer, nor with a Ok/Result type
    // see https://godot-rust.github.io/book/register/constructors.html#custom-constructors
    #[func]
    pub fn create() -> Gd<Self> {
        let base_client = BaseConvexClient::new();
        Gd::from_object(Self {
            base_client,
        })
    }

    // get pop next message
    #[func]
    pub fn pop_next_message(&mut self) -> GString {
        self.base_client.pop_next_message().map(|message| {
            // Assuming the existence of the TryFrom implementation for serde_json::Value
            let msg = serde_json::Value::try_from(message)
                .expect("Failed to serialize message") // Handle error appropriately
                .to_string();
            msg.into()
        }).unwrap_or_else(|| GString::from(""))
    }

    // receive server message
    // I am on purpose not returning anything, results are fetched by subscribers
    #[func]
    pub fn receive_message(&mut self, message: GString) {
        let msg: String = message.into(); // Convert GString to &str first, if necessary
        let msg_str: &str = &msg;
        // Attempt to convert the message into a serde_json Value, then into a ServerMessage
        println!("Receiving message, str: {:?}", msg);
        let server_message_json: JsonValue = serde_json::from_str(msg_str).expect("Failed to parse message");
        match ServerMessage::try_from(server_message_json) {
            Ok(server_message) => {
                let res = self.base_client.receive_message(server_message);
                println!("Received message: {:?}", res);
            },
            Err(e) => {
                // This will execute if there's an error in the conversion
                println!("Failed to convert msg_json to ServerMessage: {:?}", e);
            },
        }
        // Function implicitly returns (), as there's no return statement
    }

    // exposing the subsription method
    #[func]
    pub fn subscribe(
        &mut self,
        udf_path: GString,
        args: Dictionary
    ) -> Gd<Subscription> {
        // convert Gstring to UdfPath
        let udf_path_str: String = udf_path.into(); // Convert GString to &str first, if necessary
        let udf_path = match UdfPath::from_str(&udf_path_str) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Error parsing UdfPath: {}", e);
                let subscription = Subscription {
                    success: false,
                    subscriber_id: None,
                };
                return Gd::from_object(subscription); // Or handle the error as appropriate for your context
            },
        };
        // parse the args
        let args_btm = match convert_dictionary_to_btreemap(&args) {
            Ok(btm) => btm,
            Err(e) => {
                eprintln!("Error converting dictionary: {}", e);
                let subscription = Subscription {
                    success: false,
                    subscriber_id: None,
                };
                return Gd::from_object(subscription); // Or handle the error as appropriate for your context
            },
        };
        let subscriber_id = self.base_client.subscribe(
            udf_path,
            args_btm,
        );
        let subscription = Subscription {
            success: true,
            subscriber_id: Some(subscriber_id),
        };
        return Gd::from_object(subscription);
    }

    // get latest results for subscription
    #[func]
    pub fn get_results_for_subscription(&mut self, subscription: Gd<Subscription>) -> Dictionary {
        let sub = subscription.bind();
        let subscriber_id = sub.subscriber_id.expect("Subscriber ID is required but was None");
        let results_option = self.base_client.latest_results().get(&subscriber_id).cloned();
        // Convert the results to a Dictionary and return it
        convert_function_result_to_dictionary(results_option)
    }

    // implement mutation
    #[func]
    pub fn mutation(&mut self, udf_path: GString, args: Dictionary) -> Option<Gd<ResultReceiver>>{
        // convert Gstring to UdfPath
        let udf_path_str: String = udf_path.into(); // Convert GString to &str first, if necessary
        let udf_path = match UdfPath::from_str(&udf_path_str) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Error parsing UdfPath: {}", e);
                return None; // Or handle the error as appropriate for your context
            },
        };
        // parse the args
        let args_btm = match convert_dictionary_to_btreemap(&args) {
            Ok(btm) => btm,
            Err(e) => {
                eprintln!("Error converting dictionary: {}", e);
                return None; // Or handle the error as appropriate for your context
            },
        };
        let receiver = self.base_client.mutation(udf_path, args_btm);
        // return receiver 
        let result_receiver = ResultReceiver {
            success: true,
            receiver: Some(receiver),
        };
        Some(Gd::from_object(result_receiver))
    }

    // implement action
    #[func]
    pub fn action(&mut self, udf_path: GString, args: Dictionary) -> Option<Gd<ResultReceiver>>{
        // convert Gstring to UdfPath
        let udf_path_str: String = udf_path.into(); // Convert GString to &str first, if necessary
        let udf_path = match UdfPath::from_str(&udf_path_str) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Error parsing UdfPath: {}", e);
                return None; // Or handle the error as appropriate for your context
            },
        };
        // parse the args
        let args_btm = match convert_dictionary_to_btreemap(&args) {
            Ok(btm) => btm,
            Err(e) => {
                eprintln!("Error converting dictionary: {}", e);
                return None; // Or handle the error as appropriate for your context
            },
        };
        let receiver = self.base_client.action(udf_path, args_btm);
        // return receiver 
        let result_receiver = ResultReceiver {
            success: true,
            receiver: Some(receiver),
        };
        Some(Gd::from_object(result_receiver))
    }

    // 
}

fn convert_function_result_to_dictionary(results_option: Option<FunctionResult>) -> Dictionary {
    match results_option {
        Some(FunctionResult::Value(value)) => {
            // Convert the Value to a Dictionary
            let mut result_dict = Dictionary::new();
            result_dict.insert("type", "value");
            result_dict.insert("data", convex_value_to_variant(&value).unwrap_or(Variant::nil()));
            result_dict
        },
        Some(FunctionResult::ErrorMessage(message)) => {
            // Convert the error message to a Dictionary
            let mut error_dict = Dictionary::new();
            error_dict.insert("type", "error");
            error_dict.insert("message", message);
            error_dict
        },
        Some(FunctionResult::ConvexError(convex_error)) => {
            // Convert the ConvexError to a Dictionary
            let mut error_dict = Dictionary::new();
            error_dict.insert("type", "convex_error");
            error_dict.insert("message", convex_error.message);
            // Assuming convex_error.data is convertible to Variant
            error_dict.insert("data", convex_value_to_variant(&convex_error.data).unwrap_or(Variant::nil()));
            error_dict
        },
        None => {
            // Handle the case where there is no result
            let mut no_result_dict = Dictionary::new();
            no_result_dict.insert("type", "no_result");
            no_result_dict
        }
    }
}

fn convert_dictionary_to_btreemap(dictionary: &Dictionary) -> Result<BTreeMap<String, Value>, String> {
    let mut map = BTreeMap::new();

    // Iterate through the Dictionary
    for (key, value) in dictionary.iter_shared() {
        // Attempt to convert the key to a String
        let key_str_result = key.try_to::<GString>();

        let key_str: String = match key_str_result {
            Ok(godot_string) => godot_string.to_string(),
            Err(_) => {
                // Handle the error case, e.g., by logging or using a default string
                eprintln!("Failed to convert Variant to GodotString.");
                String::new() // Return a default or fallback value
            },
        };

        // Convert the Variant value to a convex::Value
        let convex_value = match variant_to_convex_value(&value) {
            Ok(convex_value) => convex_value,
            Err(e) => {
                // Handle the error, e.g., by logging or returning an error
                eprintln!("Failed to convert Variant to convex::Value: {}", e);
                return Err(e);
            },
        };

        // Insert the key-value pair into the map
        map.insert(key_str, convex_value);
    }

    Ok(map)
}

fn variant_to_convex_value(variant: &Variant) -> Result<Value, String> {
    match variant.get_type() {
        VariantType::Nil => Ok(Value::Null),
        VariantType::Bool => variant.try_to::<bool>().map(Value::Boolean).map_err(|e| e.to_string()),
        VariantType::Int => variant.try_to::<i64>().map(Value::Int64).map_err(|e| e.to_string()),
        VariantType::Float => variant.try_to::<f64>().map(Value::Float64).map_err(|e| e.to_string()),
        VariantType::String => variant.try_to::<GString>().map(|s| Value::String(s.to_string())).map_err(|e| e.to_string()),
        VariantType::PackedByteArray => variant.try_to::<PackedByteArray>().map(|ba| Value::Bytes(ba.to_vec())).map_err(|e| e.to_string()),
        VariantType::Array => {
            let array: VariantArray = variant.try_to::<VariantArray>().map_err(|e| e.to_string())?;
            let mut vec = Vec::new();
            for element in array.iter_shared() {
                vec.push(variant_to_convex_value(&element)?);
            }
            Ok(Value::Array(vec))
        },
        VariantType::Dictionary => {
            let dict: Dictionary = variant.try_to::<Dictionary>().map_err(|e| e.to_string())?;
            let mut map = BTreeMap::new();
            for (key, value) in dict.iter_shared() {
                let key_str = key.try_to::<GString>().map_err(|e| e.to_string())?.to_string();
                map.insert(key_str, variant_to_convex_value(&value)?);
            }
            Ok(Value::Object(map))
        },
        _ => Err(format!("Unsupported Variant type: {:?}", variant.get_type())),
    }
}

fn convert_btreemap_to_dictionary(btreemap: &BTreeMap<String, Value>) -> Dictionary {
    let mut dictionary = Dictionary::new();

    for (key, value) in btreemap {
        let variant_value = convex_value_to_variant(value);
        match variant_value {
            Ok(variant) => {
                dictionary.insert(key.clone(), variant);
            },
            Err(e) => {
                eprintln!("Failed to convert convex::Value to Variant: {}", e);
                // Handle the error as appropriate for your context
            },
        }
    }

    return dictionary;
}

fn convex_value_to_variant(value: &Value) -> Result<Variant, String> {
    match value {
        Value::Null => Ok(Variant::nil()),
        Value::Boolean(b) => Ok(Variant::from(*b)),
        Value::Int64(i) => Ok(Variant::from(*i)),
        Value::Float64(f) => Ok(Variant::from(*f)),
        Value::String(s) => Ok(Variant::from(s.as_str())),
        Value::Bytes(b) => {
            // convert Bytes to Vec<u8>
            let vec: Vec<u8> = b.clone().into();
            let packed_byte_array = PackedByteArray::from_iter(vec);
            // Now convert PackedByteArray to Variant
            Ok(Variant::from(packed_byte_array))
        },
        Value::Array(a) => {
            let mut array = VariantArray::new();
            for item in a {
                let variant_item = convex_value_to_variant(item)?;
                array.push(variant_item);
            }
            Ok(Variant::from(array))
        },
        Value::Object(o) => {
            let mut dict = Dictionary::new();
            for (key, val) in o {
                let variant_val = convex_value_to_variant(val)?;
                dict.insert(key.clone(), variant_val);
            }
            Ok(Variant::from(dict))
        },
    }
}

fn vec_to_array<T, const N: usize>(v: Vec<T>) -> [T; N] {
    v.try_into()
        .unwrap_or_else(|v: Vec<T>| panic!("Expected a Vec of length {} but it was {}", N, v.len()))
}