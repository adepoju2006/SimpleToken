// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod simple_token {
    use ink::storage::Mapping;
    use ink::prelude::{string::String, vec::Vec};

    #[ink(storage)]
    pub struct SimpleToken {
        balances: Mapping<AccountId, u128>,
        allowances: Mapping<(AccountId, AccountId), u128>, // (owner, spender) → allowance
        owner: AccountId,
        paused: bool,
        blacklist: Mapping<AccountId, bool>,
    }

    // Events
    #[ink(event)]
    pub struct Mint {
        #[ink(topic)]
        to: AccountId,
        amount: u128,
    }

    #[ink(event)]
    pub struct Burn {
        #[ink(topic)]
        from: AccountId,
        amount: u128,
    }

    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: u128,
    }

    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        owner: AccountId,
        #[ink(topic)]
        spender: AccountId,
        amount: u128,
    }

    impl Default for SimpleToken {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SimpleToken {
        /// Constructor
        #[ink(constructor)]
        pub fn new() -> Self {
            let caller = Self::env().caller();
            Self {
                balances: Mapping::default(),
                allowances: Mapping::default(),
                owner: caller,
                paused: false,
                blacklist: Mapping::default(),
            }
        }

        /// Internal check for pause/blacklist
        fn can_transfer(&self, from: &AccountId, to: &AccountId) -> Result<(), String> {
            if self.paused {
                return Err("Transfers are paused".into());
            }
            if self.blacklist.get(from).unwrap_or(false) {
                return Err("Sender is blacklisted".into());
            }
            if self.blacklist.get(to).unwrap_or(false) {
                return Err("Recipient is blacklisted".into());
            }
            Ok(())
        }

        /// Mint tokens (only owner)
        #[ink(message)]
        pub fn mint(&mut self, to: AccountId, amount: u128) -> Result<(), String> {
            let caller = self.env().caller();
            if caller != self.owner {
                return Err("Only the owner can mint tokens".into());
            }
            let current = self.balances.get(to).unwrap_or(0);
            let new_balance = current.saturating_add(amount);
            self.balances.insert(to, &new_balance);
            self.env().emit_event(Mint { to, amount });
            Ok(())
        }

        /// Burn own tokens
        #[ink(message)]
        pub fn burn(&mut self, amount: u128) -> Result<(), String> {
            let caller = self.env().caller();
            let balance = self.balances.get(caller).unwrap_or(0);
            if balance < amount {
                return Err("Not enough balance to burn".into());
            }
            let updated = balance.saturating_sub(amount);
            self.balances.insert(caller, &updated);
            self.env().emit_event(Burn { from: caller, amount });
            Ok(())
        }

        /// Read balance
        #[ink(message)]
        pub fn balance_of(&self, owner: AccountId) -> u128 {
            self.balances.get(owner).unwrap_or(0)
        }

        /// Transfer
        #[ink(message)]
        pub fn transfer(&mut self, to: AccountId, amount: u128) -> Result<(), String> {
            let caller = self.env().caller();
            self.can_transfer(&caller, &to)?;

            let from_balance = self.balances.get(caller).unwrap_or(0);
            if from_balance < amount {
                return Err("Not enough balance".into());
            }

            let updated_from = from_balance.saturating_sub(amount);
            self.balances.insert(caller, &updated_from);

            let to_balance = self.balances.get(to).unwrap_or(0);
            let updated_to = to_balance.saturating_add(amount);
            self.balances.insert(to, &updated_to);

            self.env().emit_event(Transfer { from: caller, to, amount });
            Ok(())
        }

        /// Approve spender
        #[ink(message)]
        pub fn approve(&mut self, spender: AccountId, amount: u128) -> Result<(), String> {
            let caller = self.env().caller();
            self.allowances.insert((caller, spender), &amount);
            self.env().emit_event(Approval {
                owner: caller,
                spender,
                amount,
            });
            Ok(())
        }

        /// Allowance query
        #[ink(message)]
        pub fn allowance(&self, owner: AccountId, spender: AccountId) -> u128 {
            self.allowances.get((owner, spender)).unwrap_or(0)
        }

        /// Transfer from (using allowance)
        #[ink(message)]
        pub fn transfer_from(
            &mut self,
            from: AccountId,
            to: AccountId,
            amount: u128,
        ) -> Result<(), String> {
            let caller = self.env().caller();
            self.can_transfer(&from, &to)?;

            let allowance = self.allowances.get((from, caller)).unwrap_or(0);
            if allowance < amount {
                return Err("Allowance too low".into());
            }

            let from_balance = self.balances.get(from).unwrap_or(0);
            if from_balance < amount {
                return Err("Not enough balance".into());
            }

            // update balances
            self.balances.insert(from, &(from_balance.saturating_sub(amount)));
            let to_balance = self.balances.get(to).unwrap_or(0);
            self.balances.insert(to, &(to_balance.saturating_add(amount)));

            // update allowance
            self.allowances.insert((from, caller), &(allowance.saturating_sub(amount)));

            self.env().emit_event(Transfer { from, to, amount });
            Ok(())
        }

        /// Pause / Unpause (owner only)
        #[ink(message)]
        pub fn set_paused(&mut self, state: bool) -> Result<(), String> {
            let caller = self.env().caller();
            if caller != self.owner {
                return Err("Only owner can pause/unpause".into());
            }
            self.paused = state;
            Ok(())
        }

        /// Blacklist / Unblacklist (owner only)
        #[ink(message)]
        pub fn set_blacklist(&mut self, account: AccountId, state: bool) -> Result<(), String> {
            let caller = self.env().caller();
            if caller != self.owner {
                return Err("Only owner can manage blacklist".into());
            }
            self.blacklist.insert(account, &state);
            Ok(())
        }

        /// Batch transfers
        #[ink(message)]
        pub fn batch_transfer(
            &mut self,
            recipients: Vec<AccountId>,
            amounts: Vec<u128>,
        ) -> Result<(), String> {
            let caller = self.env().caller();
            if recipients.len() != amounts.len() {
                return Err("Mismatched input lengths".into());
            }

            for i in 0..recipients.len() {
                self.transfer(recipients[i], amounts[i])?;
            }
            Ok(())
        }
    }
}
