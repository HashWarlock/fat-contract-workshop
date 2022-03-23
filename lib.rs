#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use pink_extension as pink;

#[pink::contract(env=PinkEnvironment)]
mod phat_messenger {
    use super::pink;
    use pink::{PinkEnvironment, http_post, http_get};
    use alloc::{string::String, vec::Vec};

    #[ink(storage)]
    pub struct PhatMessenger {}

    impl PhatMessenger {
        #[ink(constructor)]
        pub fn default() -> Self {
            Self {}
        }

        #[ink(message)]
        pub fn get_data(&self, url: String) -> (u16, Vec<u8>) {
            let response = http_get!(url);
            (response.status_code, response.body)
        }

        #[ink(message)]
        pub fn post_data(&self, url: String, body: Vec<u8>, headers: Vec<(String, String)>) -> (u16, Vec<u8>) {
            let response = http_post!(url, body, headers);
            (response.status_code, response.body)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink_lang as ink;
        use pink_extension::chain_extension::HttpRequest;
        use alloc::{string::{String, ToString}};

        #[ink::test]
        fn get_data_works() {
            use pink_extension::chain_extension::{mock, HttpResponse};

            mock::mock_http_request(|request| {
                if request.url == "https://ip.kvin.wang" {
                    HttpResponse::ok(b"1.1.1.1".to_vec())
                } else {
                    HttpResponse::not_found()
                }
            });

            let contract = PhatMessenger::default();
            //assert_eq!(contract.get_data("https://ip.kvin.wang".to_string()).1, b"1.1.1.1");
        }
    }
}