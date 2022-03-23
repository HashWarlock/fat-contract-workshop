#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use pink_extension as pink;

#[ink::contract]
mod auction_house {
    use super::pink;
    use pink::{PinkEnvironment, http_post, http_get};
    use ink_env::{DefaultEnvironment};
    use ink_storage::traits::{
        SpreadAllocate,
        SpreadLayout,
        StorageLayout
    };
    use scale::{Decode, Encode};
    use alloc::{
        string::{String, ToString},
        vec::Vec, format
    };
    use phat_messenger::PhatMessengerRef;

    /// Messenger structure
    #[derive(Debug, Eq, PartialEq)]
    pub struct MessengerBot {
        headers: Vec<(String, String)>,
        text: String,
        url: String,
        chat_id: String,
    }

    /// Auction structure
    #[derive(Default, Debug, Clone, PartialEq, Encode, Decode, SpreadLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout))]
    pub struct Auction {
        /// Owner of the Auction
        owner: AccountId,
        /// ID for the RMRK NFT
        token_id: String,
        /// The current highest bid amount
        amount: u128,
        /// The time the action started
        start_time: Timestamp,
        /// The time that the auction is scheduled to end
        end_time: Timestamp,
        /// The address of the current highest bid
        bidder: Option<AccountId>,
        /// Whether the auction is settled
        settled: bool,
    }

    /// Auction House
    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct AuctionHouse {
        /// Auction House Owner
        owner: AccountId,
        /// Auctions mapping by Token ID
        token_auctions: Mapping<String, Auction>,
        /// The minimum of time left after a new bid is created
        time_buffer: u64,
        /// The minimum price accepted in an auction
        reserve_price: u128,
        /// The minimum percentage increase between bids
        min_bid_increment_percentage: u128,
        /// The duration of a single auction
        duration: u64,
        /// Phat Messenger contract reference
        phat_messenger_ref: PhatMessengerRef,
    }

    #[ink(event)]
    pub struct AuctionCreated {
        token_id: TokenId,
        start_time: Timestamp,
        end_time: Timestamp,
    }

    #[ink(event)]
    pub struct AuctionBid {
        token_id: TokenId,
        sender: AccountId,
        amount: Balance,
        extended: bool,
    }

    #[ink(event)]
    pub struct AuctionExtended {
        token_id: TokenId,
        end_time: Timestamp,
    }

    #[ink(event)]
    pub struct AuctionSettled {
        token_id: TokenId,
        winner: Option<AccountId>,
        amount: Balance,
    }

    #[ink(event)]
    pub struct AuctionTimeBufferUpdated {
        time_buffer: u64,
    }

    #[ink(event)]
    pub struct AuctionReservePriceUpdated {
        reserve_price: Balance,
    }

    #[ink(event)]
    pub struct AuctionMinBidIncrementPercentageUpdated {
        min_bid_increment_percentage: u128,
    }

    #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq, Copy, Clone)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        NotOwner,
        NotApproved,
        OwnerCannotBidOnToken,
        TokenNotForAuction,
        TokenAuctionExpired,
        BidBelowReservePrice,
        BidBelowMinBidIncrementPercentage,
        TokenAuctionHasNotStarted,
        TokenAuctionHasBeenSettled,
        TokenAuctionStillInProgress,
        TokenAuctionHasNotFound,
        BidderAlreadyTopBid,
    }

    impl AuctionHouse {
        /// Default Constructor to Initialize Auction House
        #[ink(constructor)]
        pub fn default() -> Self {
            // Save sender as the contract admin
            let owner = Self::env().caller();
            // Hash of the PhatMessenger contract
            let hash = hex!("a3f91e98edc8ccfb035946133027dd5a3f8694c70e7a27ffdf8056f7b9cc40ab").into();
            let phat_messenger_ref = PhatMessengerRef::default()
                .endowment(100000)
                .salt_bytes(&[0x00])
                .code_hash(hash)
                .instantiate()
                .expect("failed at instantiating the `PhatMessengerRef` contract..");
            // This call is required in order to correctly initialize the
            // `Mapping`s of our contract.
            ink_lang::codegen::initialize_contract(|contract: &mut Self| {
                contract.owner = owner;
                contract.time_buffer = Default::default();
                contract.reserve_price = Default::default();
                contract.min_bid_increment_percentage = Default::default();
                contract.duration = Default::default();
                contract.phat_messenger_ref = phat_messenger_ref;
            })
        }
        /// Constructor that initializes the Auction House
        #[ink(constructor)]
        pub fn new(
            _token_contract: TokenId,
            _time_buffer: u64,
            _reserve_price: Balance,
            _min_bid_increment_percentage: u128,
            _duration: u64,
        ) -> Self {
            // TODO:
            // 1) Init Pausible
            // 2) Reentrancy Guard
            // 3) Init Ownable
            // 4) Pause
            Self {
                owner: Self::env().caller(),
                token_contract: _token_contract,
                time_buffer: _time_buffer,
                reserve_price: _reserve_price,
                min_bid_increment_percentage: _min_bid_increment_percentage,
                duration: _duration,
                token_auction: None,
            }
        }

        // TODO: reentrancy guard from OpenBrush
        #[ink(message)]
        pub fn settle_current_and_create_new_auction(&mut self, token_id: TokenId) {
            Self::_settle_auction(self);
            Self::_create_auction(self, token_id);
        }

        #[ink(message)]
        pub fn settle_auction(&mut self) {
            Self::_settle_auction(self);
        }

        #[ink(message)]
        pub fn create_bid(
            &mut self,
            token_id: TokenId,
            amount: Balance
        ) -> Result<(), Error> {
            if let Some (mut auction) = self.token_auction.clone() {
                if auction.token_id != token_id { return Err(Error::TokenNotForAuction); }
                if self.env().block_timestamp() < auction.end_time { return Err(Error::TokenAuctionExpired); }
                if self.reserve_price <= amount { return Err(Error::BidBelowReservePrice); }
                if amount >= auction.amount +
                    ((auction.amount * self.min_bid_increment_percentage) / 100) {
                    return Err(Error::BidBelowMinBidIncrementPercentage);
                }

                let sender = self.env().caller();
                if sender != self.owner { return Err(Error::OwnerCannotBidOnToken); }

                let last_bidder = auction.bidder;
                if last_bidder.is_none() {
                    // TODO: Refund the last bidder
                }

                if last_bidder != Some(sender) { return Err(Error::BidderAlreadyTopBid); }

                auction.amount = amount;
                auction.bidder = Some(sender.clone());
                // Extend auction if bad received within time_buffer of the auction end time
                let extended = auction.end_time - self.env().block_timestamp() < self.time_buffer;
                if extended {
                    auction.end_time = self.env().block_timestamp() + self.time_buffer;
                }

                self.token_auction = Some(auction.clone());

                self.env().emit_event(AuctionBid{
                    token_id,
                    sender,
                    amount,
                    extended,
                });

                if extended {
                    self.env().emit_event(AuctionExtended{
                        token_id,
                        end_time: auction.end_time,
                    });
                }

                Ok(())

            } else {
                return Err(Error::TokenAuctionHasNotFound);
            }
        }

        // TODO: Access Control
        //#[ink(message)]
        //pub fn pause() {
            //Self::_pause();
        //}

        //#[ink(message)]
        //pub fn unpause(&mut self, token_id: TokenId) {
        //    Self::_unpause();

        //    if let Some(auction) = self.token_auction.clone() {
        //        if auction.start_time == 0 || auction.settled {
        //            Self::_create_auction(self, token_id);
        //        }
        //    }
        //}

        #[ink(message)]
        pub fn set_time_buffer(&mut self, time_buffer: Timestamp) {
            // TODO Access Control
            self.time_buffer = time_buffer;

            self.env().emit_event(AuctionTimeBufferUpdated{
                time_buffer,
            });
        }

        #[ink(message)]
        pub fn set_reserve_price(&mut self, reserve_price: Balance) {
            // TODO Access Control
            self.reserve_price = reserve_price;

            self.env().emit_event(AuctionReservePriceUpdated{
                reserve_price,
            });
        }

        #[ink(message)]
        pub fn set_min_bid_increment_percentage(&mut self, min_bid_increment_percentage: u128) {
            // TODO Access Control
            self.min_bid_increment_percentage = min_bid_increment_percentage;

            self.env().emit_event(AuctionMinBidIncrementPercentageUpdated{
                min_bid_increment_percentage,
            });
        }

        // Internal functions
        fn _create_auction(&mut self, token_id: TokenId) {
            let start_time = self.env().block_timestamp();
            let end_time = start_time + self.duration;

            let auction = Auction {
                token_id,
                amount: 0,
                start_time,
                end_time,
                bidder: None,
                settled: false,
            };

            self.token_auction = Some(auction);

            self.env().emit_event(AuctionCreated{
                token_id,
                start_time,
                end_time
            });
        }

        fn _settle_auction(&mut self) -> Result<(), Error> {
            if let Some(mut auction) = self.token_auction.clone() {
                if auction.start_time != 0 { return Err(Error::TokenAuctionHasNotStarted); }
                if !auction.settled { return Err(Error::TokenAuctionHasBeenSettled); }
                if self.env().block_timestamp() >= auction.end_time {
                    return Err(Error::TokenAuctionStillInProgress);
                }

                auction.settled = true;

                if auction.bidder.is_none() {
                    // TODO: burn NFT
                } else {
                    // Transfer NFT to new owner
                }

                if auction.amount > 0 {
                    // Perform Balance transfer
                    // _transfer_currency(to: AccountId, amount: Balance);
                }

                self.env().emit_event(AuctionSettled{
                    token_id: auction.token_id,
                    winner: auction.bidder,
                    amount: auction.amount,
                });

                Ok(())

            } else {
                return Err(Error::TokenAuctionHasNotFound);
            }

        }

    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// Imports `ink_lang` so we can use `#[ink::test]`.
        use ink_lang as ink;

        /// We test if the default constructor does its job.
        #[ink::test]
        fn default_works() {
            assert_eq!(false, false);
        }

        /// We test a simple use case of our contract.
        #[ink::test]
        fn it_works() {
            assert_eq!(false, false);
        }
    }
}
