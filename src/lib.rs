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
use base64::{
    engine::general_purpose::{self, GeneralPurpose},
    Engine,
};
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::{
    FungibleToken, FungibleTokenCore, FungibleTokenResolver,
};
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::json_types::Base64VecU8;
use near_sdk::json_types::U128;
use near_sdk::store::LazyOption;
use near_sdk::{borsh::BorshSerialize, require};
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
    owner: AccountId,
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
    #[private]
    #[init]
    pub fn new_default_meta(owner_id: AccountId, total_supply: U128) -> Self {
        let engine: GeneralPurpose = general_purpose::STANDARD;
        let decoded: Vec<u8> = engine
            .decode("K29udivYwweOUnCZPFt/KhcMmm0DQLvzYoVdKXN41P8=")
            .expect("ERR_FAILED_TO_DECODE_REFERENCE_HASH");
        Self::new(
            owner_id,
            total_supply,
            FungibleTokenMetadata {
                spec: FT_METADATA_SPEC.to_string(),
                name: "Intellex AI Protocol Token".to_string(),
                symbol: "ITLX".to_string(),
                icon: Some(DATA_IMAGE_SVG_ITLX_ICON.to_string()),
                reference: Some("https://raw.githubusercontent.com/brainstems/itlx_nep141_token/refs/heads/master/metadata.json".to_string()),
                reference_hash: Some(Base64VecU8::from(decoded)),
                decimals: 24,
            },
        )
    }

    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// the given fungible token metadata.
    #[private]
    #[init]
    pub fn new(owner_id: AccountId, total_supply: U128, metadata: FungibleTokenMetadata) -> Self {
        metadata.assert_valid();
        let mut this = Self {
            token: FungibleToken::new(StorageKey::FungibleToken),
            metadata: LazyOption::new(StorageKey::Metadata, Some(metadata)),
            session_vault_id: None,
            owner: env::signer_account_id(),
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
        require!(env::predecessor_account_id().eq(&self.owner));
        self.session_vault_id = Some(session_vault_id);
    }
}

#[near]
impl FungibleTokenCore for Contract {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        if let Some(session_vault_id) = self.session_vault_id.as_ref() {
            assert_ne!(
                receiver_id, *session_vault_id,
                "ERR_RECIPIENT_CANNOT_BE_SESSION_VAULT"
            );
        }
        self.token.ft_transfer(receiver_id, amount, memo)
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
        self.metadata.get().clone().unwrap()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_contract_standards::fungible_token::Balance;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, Gas};

    use super::*;

    const TOTAL_SUPPLY: Balance = 1_000_000_000_000_000;

    fn current() -> AccountId {
        accounts(0)
    }

    fn owner() -> AccountId {
        accounts(1)
    }

    fn user1() -> AccountId {
        accounts(2)
    }

    fn user2() -> AccountId {
        accounts(3)
    }

    fn setup() -> (Contract, VMContextBuilder) {
        let mut context = VMContextBuilder::new();

        let contract = Contract::new_default_meta(owner(), TOTAL_SUPPLY.into());

        context.storage_usage(env::storage_usage());
        context.current_account_id(current());

        testing_env!(context.build());

        (contract, context)
    }

    #[test]
    fn test_new() {
        let (contract, _) = setup();

        assert_eq!(contract.ft_total_supply().0, TOTAL_SUPPLY);
        assert_eq!(contract.ft_balance_of(owner()).0, TOTAL_SUPPLY);
    }

    #[test]
    fn test_metadata() {
        let (contract, _) = setup();

        assert_eq!(contract.ft_metadata().decimals, 24);
        assert!(contract.ft_metadata().icon.is_some());
        assert!(!contract.ft_metadata().spec.is_empty());
        assert!(!contract.ft_metadata().name.is_empty());
        assert!(!contract.ft_metadata().symbol.is_empty());
    }

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn test_default_panics() {
        Contract::default();
    }

    #[test]
    fn test_deposit() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        assert!(contract.storage_balance_of(user1()).is_none());

        contract.storage_deposit(None, None);

        let storage_balance = contract.storage_balance_of(user1()).unwrap();
        assert_eq!(storage_balance.total, contract.storage_balance_bounds().min);
        assert!(storage_balance.available.is_zero());
    }

    #[test]
    fn test_deposit_on_behalf_of_another_user() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        assert!(contract.storage_balance_of(user2()).is_none());

        // predecessor is user1, but deposit is for user2
        contract.storage_deposit(Some(user2()), None);

        let storage_balance = contract.storage_balance_of(user2()).unwrap();
        assert_eq!(storage_balance.total, contract.storage_balance_bounds().min);
        assert!(storage_balance.available.is_zero());

        // ensure that user1's storage wasn't affected
        assert!(contract.storage_balance_of(user1()).is_none());
    }

    #[should_panic]
    #[test]
    fn test_deposit_panics_on_less_amount() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(100))
            .build());

        assert!(contract.storage_balance_of(user1()).is_none());

        // this panics
        contract.storage_deposit(None, None);
    }

    #[test]
    fn test_deposit_account_twice() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // this registers the predecessor
        contract.storage_deposit(None, None);

        let storage_balance = contract.storage_balance_of(user1()).unwrap();
        assert_eq!(storage_balance.total, contract.storage_balance_bounds().min);

        // this doesn't panic, and just refunds the deposit as the account is registered already
        contract.storage_deposit(None, None);

        // this indicates that total balance hasn't changed
        let storage_balance = contract.storage_balance_of(user1()).unwrap();
        assert_eq!(storage_balance.total, contract.storage_balance_bounds().min);
    }

    #[test]
    fn test_unregister() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        contract.storage_deposit(None, None);

        assert!(contract.storage_balance_of(user1()).is_some());

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        assert!(contract.storage_unregister(None));

        assert!(contract.storage_balance_of(user1()).is_none());
    }

    #[should_panic]
    #[test]
    fn test_unregister_panics_on_zero_deposit() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        contract.storage_deposit(None, None);

        assert!(contract.storage_balance_of(user1()).is_some());

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(0))
            .build());

        contract.storage_unregister(None);
    }

    #[test]
    fn test_unregister_of_non_registered_account() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        // "false" indicates that the account wasn't registered
        assert!(!contract.storage_unregister(None));
    }

    #[should_panic]
    #[test]
    fn test_unregister_panics_on_non_zero_balance() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        contract.storage_deposit(None, None);

        assert!(contract.storage_balance_of(user1()).is_some());

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());
        let transfer_amount = TOTAL_SUPPLY / 10;

        contract.ft_transfer(user1(), transfer_amount.into(), None);

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        contract.storage_unregister(None);
    }

    #[test]
    fn test_unregister_with_force() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        contract.storage_deposit(None, None);

        assert!(contract.storage_balance_of(user1()).is_some());

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());
        let transfer_amount = TOTAL_SUPPLY / 10;

        contract.ft_transfer(user1(), transfer_amount.into(), None);

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        // force to unregister no matter what
        // this reduces total supply because user's tokens are burnt
        assert!(contract.storage_unregister(Some(true)));

        assert!(contract.storage_balance_of(user1()).is_none());
        assert_eq!(contract.ft_balance_of(user1()).0, 0);
        assert_eq!(contract.ft_total_supply().0, TOTAL_SUPPLY - transfer_amount);
    }

    #[test]
    fn test_withdraw() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        // Basic Fungible Token implementation never transfers Near to caller
        // See: https://github.com/near/near-sdk-rs/blob/5a4c595125364ffe8d7866aa0418a3c92b1c3a6a/near-contract-standards/src/fungible_token/storage_impl.rs#L82
        let storage_balance = contract.storage_withdraw(None);
        assert_eq!(storage_balance.total, contract.storage_balance_bounds().min);
        assert!(storage_balance.available.is_zero());

        // Basic Fungible Token implementation never transfers Near to caller
        // See: https://github.com/near/near-sdk-rs/blob/5a4c595125364ffe8d7866aa0418a3c92b1c3a6a/near-contract-standards/src/fungible_token/storage_impl.rs#L82
        let storage_balance = contract.storage_withdraw(None);
        assert_eq!(storage_balance.total, contract.storage_balance_bounds().min);
        assert!(storage_balance.available.is_zero());
    }

    #[should_panic]
    #[test]
    fn test_withdraw_panics_on_non_registered_account() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        contract.storage_withdraw(None);
    }

    #[should_panic]
    #[test]
    fn test_withdraw_panics_on_zero_deposit() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(0))
            .build());

        contract.storage_withdraw(None);
    }

    #[should_panic]
    #[test]
    fn test_withdraw_panics_on_amount_greater_than_zero() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        // Basic Fungible Token implementation sets storage_balance_bounds.min == storage_balance_bounds.max
        // which means available balance will always be 0
        // See: https://github.com/near/near-sdk-rs/blob/5a4c595125364ffe8d7866aa0418a3c92b1c3a6a/near-contract-standards/src/fungible_token/storage_impl.rs#L82
        contract.storage_withdraw(Some(NearToken::from_yoctonear(1)));
    }

    #[test]
    fn test_transfer() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());
        let transfer_amount = TOTAL_SUPPLY / 10;

        contract.ft_transfer(user1(), transfer_amount.into(), None);

        assert_eq!(
            contract.ft_balance_of(owner()).0,
            (TOTAL_SUPPLY - transfer_amount)
        );
        assert_eq!(contract.ft_balance_of(user1()).0, transfer_amount);
    }

    #[should_panic]
    #[test]
    fn test_transfer_panics_on_self_receiver() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());
        let transfer_amount = TOTAL_SUPPLY / 10;

        contract.ft_transfer(owner(), transfer_amount.into(), None);
    }

    #[should_panic]
    #[test]
    fn test_transfer_panics_on_zero_amount() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        contract.ft_transfer(user1(), 0.into(), None);
    }

    #[should_panic]
    #[test]
    fn test_transfer_panics_on_zero_deposit() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(0))
            .build());

        let transfer_amount = TOTAL_SUPPLY / 10;
        contract.ft_transfer(user1(), transfer_amount.into(), None);
    }

    #[should_panic]
    #[test]
    fn test_transfer_panics_on_non_registered_sender() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        let transfer_amount = TOTAL_SUPPLY / 10;
        contract.ft_transfer(user1(), transfer_amount.into(), None);
    }

    #[should_panic]
    #[test]
    fn test_transfer_panics_on_non_registered_receiver() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        let transfer_amount = TOTAL_SUPPLY / 10;
        contract.ft_transfer(user1(), transfer_amount.into(), None);
    }

    #[should_panic]
    #[test]
    fn test_transfer_panics_on_amount_greater_than_balance() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        let transfer_amount = TOTAL_SUPPLY + 10;
        contract.ft_transfer(user1(), transfer_amount.into(), None);
    }

    #[test]
    fn test_transfer_call() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());
        let transfer_amount = TOTAL_SUPPLY / 10;

        contract.ft_transfer_call(user1(), transfer_amount.into(), None, "".to_string());

        assert_eq!(
            contract.ft_balance_of(owner()).0,
            (TOTAL_SUPPLY - transfer_amount)
        );
        assert_eq!(contract.ft_balance_of(user1()).0, transfer_amount);
    }

    #[should_panic]
    #[test]
    fn test_transfer_call_panics_on_self_receiver() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());
        let transfer_amount = TOTAL_SUPPLY / 10;

        contract.ft_transfer_call(owner(), transfer_amount.into(), None, "".to_string());
    }

    #[should_panic]
    #[test]
    fn test_transfer_call_panics_on_zero_amount() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        contract.ft_transfer_call(user1(), 0.into(), None, "".to_string());
    }

    #[should_panic]
    #[test]
    fn test_transfer_call_panics_on_zero_deposit() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(0))
            .build());

        let transfer_amount = TOTAL_SUPPLY / 10;
        contract.ft_transfer_call(user1(), transfer_amount.into(), None, "".to_string());
    }

    #[should_panic]
    #[test]
    fn test_transfer_call_panics_on_non_registered_sender() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        let transfer_amount = TOTAL_SUPPLY / 10;
        contract.ft_transfer_call(user1(), transfer_amount.into(), None, "".to_string());
    }

    #[should_panic]
    #[test]
    fn test_transfer_call_panics_on_non_registered_receiver() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        let transfer_amount = TOTAL_SUPPLY / 10;
        contract.ft_transfer_call(user1(), transfer_amount.into(), None, "".to_string());
    }

    #[should_panic]
    #[test]
    fn test_transfer_call_panics_on_amount_greater_than_balance() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());

        let transfer_amount = TOTAL_SUPPLY + 10;
        contract.ft_transfer_call(user1(), transfer_amount.into(), None, "".to_string());
    }
    #[should_panic]
    #[test]
    fn test_transfer_call_panics_on_unsufficient_gas() {
        let (mut contract, mut context) = setup();

        testing_env!(context
            .predecessor_account_id(user1())
            .attached_deposit(contract.storage_balance_bounds().min)
            .build());

        // Paying for account registration of user1, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .predecessor_account_id(owner())
            .attached_deposit(NearToken::from_yoctonear(1))
            .prepaid_gas(Gas::from_tgas(10))
            .build());
        let transfer_amount = TOTAL_SUPPLY / 10;

        contract.ft_transfer_call(user1(), transfer_amount.into(), None, "".to_string());
    }
}
