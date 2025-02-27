/*!
Fungible Token implementation with JSON serialization.
NOTES:
  - The maximum balance value is limited by U128 (2**128 - 1).
  - JSON calls should pass U128 as a base-10 string. E.g. "100".
  - The contract optimizes the inner trie structure by hashing account IDs. It will prevent some
    abuse of deep tries. Shouldn't be an issue, once NEAR clients implement full hashing of keys.
  - The contract tracks the change in storage before and after the call. If the storage increases,
    the contract requires the caller of the contract to attach enough deposit to the function call
    to cover the storage cost.
    This is done to prevent a denial of service attack on the contract by taking all available storage.
    If the storage decreases, the contract will issue a refund for the cost of the released storage.
    The unused tokens from the attached deposit are also refunded, so it's safe to
    attach more deposit than required.
  - To prevent the deployed contract from being modified or deleted, it should not have any access
    keys on its account.
*/
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::{
    FungibleToken, FungibleTokenCore, FungibleTokenResolver,
};
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::borsh::BorshSerialize;
use near_sdk::collections::LazyOption;
use near_sdk::json_types::U128;
use near_sdk::json_types::Base64VecU8;
use near_sdk::{
    env, log, near, AccountId, BorshStorageKey, NearToken, PanicOnDefault, PromiseOrValue,
}; 

const DATA_IMAGE_SVG_ITLX_ICON: &str = "data:image/svg+xml,%3Csvg version='1.0' xmlns='http://www.w3.org/2000/svg' width='721.000000pt' height='399.000000pt' viewBox='0 0 721.000000 399.000000' preserveAspectRatio='xMidYMid meet'%3E%3Cg transform='translate(0.000000,399.000000) scale(0.100000,-0.100000)' fill='%23000000' stroke='none'%3E%3Cpath d='M0 1995 l0 -1995 3605 0 3605 0 0 1995 0 1995 -3605 0 -3605 0 0 -1995z m2888 1200 c110 -22 190 -64 252 -132 183 -200 178 -507 -15 -830 -75 -126 -101 -152 -50 -49 163 327 192 597 83 769 -58 91 -160 160 -277 187 -81 19 -231 15 -351 -10 -134 -27 -260 -74 -438 -161 l-143 -71 46 -50 c57 -63 109 -151 137 -231 32 -89 32 -263 1 -362 -70 -221 -249 -381 -473 -421 -129 -23 -268 -7 -325 38 -34 27 -65 92 -65 138 0 83 188 426 362 660 l33 45 -64 -50 c-342 -266 -660 -644 -817 -970 -168 -350 -171 -585 -9 -734 65 -59 135 -87 243 -100 307 -34 733 104 1261 408 60 34 45 14 -42 -57 -438 -358 -1180 -536 -1521 -365 -69 34 -140 111 -167 181 -34 85 -32 269 4 405 66 249 202 520 394 786 9 12 8 31 -3 81 -18 85 -17 229 1 309 38 159 150 298 298 370 178 87 378 93 570 16 l68 -28 97 46 c345 161 680 228 910 182z'/%3E%3C/g%3E%3C/svg%3E";

#[derive(PanicOnDefault)]
#[near(contract_state)]
pub struct Contract {
    token: FungibleToken,
    metadata: LazyOption<FungibleTokenMetadata>,
    session_vault_id: Option<AccountId>,
    owner_id: AccountId,
}

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    FungibleToken,
    Metadata,
}

#[near]
impl Contract {
    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// default metadata (for example purposes only).
    #[init]
    pub fn new_default_meta(owner_id: AccountId, total_supply: U128) -> Self {
        Self::new(
            owner_id,
            total_supply,
            FungibleTokenMetadata {
                spec: FT_METADATA_SPEC.to_string(),
                name: "Intellex AI Protocol Token (TESTING)".to_string(),
                symbol: "ITLX2".to_string(),
                icon: Some(DATA_IMAGE_SVG_ITLX_ICON.to_string()),
                reference: Some("https://raw.githubusercontent.com/brainstems/itlx_nep141_token/refs/heads/master/metadata.json".to_string()),
                reference_hash: Some(Base64VecU8::from(base64::decode("K29udivYwweOUnCZPFt/KhcMmm0DQLvzYoVdKXN41P8=").unwrap())),
                decimals: 24,
            },
        )
    }

    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// the given fungible token metadata.
    #[init]
    pub fn new(owner_id: AccountId, total_supply: U128, metadata: FungibleTokenMetadata) -> Self {
        metadata.assert_valid();
        let mut this = Self {
            token: FungibleToken::new(StorageKey::FungibleToken),
            metadata: LazyOption::new(StorageKey::Metadata, Some(&metadata)),
            session_vault_id: None,
            owner_id: owner_id.clone(),
        };
        this.token.internal_register_account(&owner_id);
        this.token.internal_deposit(&owner_id, total_supply.into());

        near_contract_standards::fungible_token::events::FtMint {
            owner_id: &owner_id,
            amount: total_supply,
            memo: Some("new tokens are minted"),
        }
        .emit();

        this
    }

    pub fn set_session_vault_id(&mut self, session_vault_id: AccountId) {
        self.assert_owner();
        self.session_vault_id = Some(session_vault_id);
    }
}

#[near]
impl FungibleTokenCore for Contract {
    #[payable]
    fn ft_transfer(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>
    ) {
        if let Some(vault_id) = &self.session_vault_id {
            if &receiver_id == vault_id {
                env::panic_str("Direct transfers to session vault are not allowed. Use ft_transfer_call instead.");
            }
        }
        
        self.token.ft_transfer(receiver_id, amount, memo);
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.token.ft_transfer_call(receiver_id, amount, memo, msg)
    }

    fn ft_total_supply(&self) -> U128 {
        self.token.ft_total_supply()
    }

    fn ft_balance_of(&self, account_id: AccountId) -> U128 {
        self.token.ft_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenResolver for Contract {
    #[private]
    fn ft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> U128 {
        let (used_amount, burned_amount) =
            self.token
                .internal_ft_resolve_transfer(&sender_id, receiver_id, amount);
        if burned_amount > 0 {
            log!("Account @{} burned {}", sender_id, burned_amount);
        }
        used_amount.into()
    }
}

#[near]
impl StorageManagement for Contract {
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        self.token.storage_deposit(account_id, registration_only)
    }

    #[payable]
    fn storage_withdraw(&mut self, amount: Option<NearToken>) -> StorageBalance {
        self.token.storage_withdraw(amount)
    }

    #[payable]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        #[allow(unused_variables)]
        if let Some((account_id, balance)) = self.token.internal_storage_unregister(force) {
            log!("Closed @{} with {}", account_id, balance);
            true
        } else {
            false
        }
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        self.token.storage_balance_bounds()
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.token.storage_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        self.metadata.get().unwrap()
    }
}

trait Ownable {
    fn assert_owner(&self);
}

impl Ownable for Contract {
    fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Method can only be called by the owner"
        );
    }
}

impl Contract {
    fn internal_transfer(
        &mut self,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        amount: u128,
        memo: Option<String>
    ) {
        self.token.internal_transfer(sender_id, receiver_id, amount, memo);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, VMContext};

    fn get_context(predecessor_account_id: AccountId) -> VMContext {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder.build()
    }

    fn setup_contract() -> (VMContext, Contract) {
        let context = get_context(accounts(0));
        testing_env!(context.clone());
        
        let mut contract = Contract::new_default_meta(accounts(0), U128(1_000_000));
        
        // Set the session vault ID
        contract.set_session_vault_id(accounts(3));
        
        (context, contract)
    }

    #[test]
    fn test_normal_transfer() {
        let (mut context, mut contract) = setup_contract();
        
        // Set predecessor as account(1) who has tokens
        context.predecessor_account_id = accounts(1);
        testing_env!(context.clone());
        
        // Test normal transfer to account(2)
        contract.ft_transfer(accounts(2), U128(100), None);
        
        // Verify balances
        assert_eq!(contract.ft_balance_of(accounts(1)), U128(999900));
        assert_eq!(contract.ft_balance_of(accounts(2)), U128(100));
    }
    
    #[test]
    #[should_panic(expected = "Direct transfers to session vault are not allowed")]
    fn test_direct_transfer_to_vault_blocked() {
        let (mut context, mut contract) = setup_contract();
        
        // Set predecessor as account(1) who has tokens
        context.predecessor_account_id = accounts(1);
        testing_env!(context.clone());
        
        // This should panic as direct transfers to vault are not allowed
        contract.ft_transfer(accounts(3), U128(100), None);
    }
    
    #[test]
    fn test_transfer_call_to_vault_works() {
        let (mut context, mut contract) = setup_contract();
        
        // Set predecessor as account(1) who has tokens
        context.predecessor_account_id = accounts(1);
        testing_env!(context.clone());
        
        // Mock successful ft_transfer_call 
        // In a real test we would need more complex setup to test cross-contract calls
        contract.ft_transfer_call(accounts(3), U128(100), None, "deposit".to_string());
        
        // In a real scenario, the on_transfer would be called by the receiving contract
        // Here we're just checking the balance was deducted from sender
        assert_eq!(contract.ft_balance_of(accounts(1)), U128(999900));
    }
    
    #[test]
    fn test_no_vault_configured() {
        let (mut context, mut contract) = setup_contract();
        
        // Reset the session vault ID to None
        contract.session_vault_id = None;
        
        // Set predecessor as account(1) who has tokens
        context.predecessor_account_id = accounts(1);
        testing_env!(context.clone());
        
        // This should work since no vault is configured
        contract.ft_transfer(accounts(3), U128(100), None);
        
        // Verify balances
        assert_eq!(contract.ft_balance_of(accounts(1)), U128(999900));
        assert_eq!(contract.ft_balance_of(accounts(3)), U128(100));
    }
    
    #[test]
    #[should_panic(expected = "Must be owner")]
    fn test_only_owner_can_set_vault() {
        let (mut context, mut contract) = setup_contract();
        
        // Try to set vault from non-owner account
        context.predecessor_account_id = accounts(1);
        testing_env!(context.clone());
        
        // This should panic as only owner can set the vault ID
        contract.set_session_vault_id(accounts(4));
    }
}
