#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    Address, Env,
};

// ──────────────────────────────────────────────
//  Composite Storage Keys
// ──────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    PoolLocked,
    PoolDisbursed,
    PerSemesterAmt,
    MinGpaScaled,
    MaxSemesters,
    Student(u64),
    Disbursed(u64, u32),
}

// ──────────────────────────────────────────────
//  On-chain Student Record
// ──────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Student {
    pub id: u64,
    pub gpa_scaled: u32,
    pub current_semester: u32,
    pub total_semesters: u32,
    pub is_active: bool,
    pub total_received: u64,
}

// ──────────────────────────────────────────────
//  Contract
// ──────────────────────────────────────────────

#[contract]
pub struct ScholarshipContract;

#[contractimpl]
impl ScholarshipContract {

    pub fn initialize(
        env: Env,
        admin: Address,
        total_funds: u64,
        per_semester_amount: u64,
        min_gpa_scaled: u32,
        max_semesters: u32,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin,          &admin);
        env.storage().instance().set(&DataKey::PoolLocked,     &total_funds);
        env.storage().instance().set(&DataKey::PoolDisbursed,  &0u64);
        env.storage().instance().set(&DataKey::PerSemesterAmt, &per_semester_amount);
        env.storage().instance().set(&DataKey::MinGpaScaled,   &min_gpa_scaled);
        env.storage().instance().set(&DataKey::MaxSemesters,   &max_semesters);
    }

    pub fn register_student(
        env: Env,
        caller: Address,
        student_id: u64,
        total_semesters: u32,
    ) {
        caller.require_auth();
        Self::assert_admin(&env, &caller);
        if env.storage().persistent().has(&DataKey::Student(student_id)) {
            panic!("student already registered");
        }
        env.storage().persistent().set(&DataKey::Student(student_id), &Student {
            id: student_id,
            gpa_scaled: 0,
            current_semester: 0,
            total_semesters,
            is_active: true,
            total_received: 0,
        });
    }

    pub fn update_gpa(env: Env, caller: Address, student_id: u64, gpa_scaled: u32) {
        caller.require_auth();
        Self::assert_admin(&env, &caller);
        if gpa_scaled > 400 {
            panic!("gpa must be <= 4.00 (400 scaled)");
        }
        let mut student: Student = env.storage().persistent()
            .get(&DataKey::Student(student_id)).expect("student not found");
        student.gpa_scaled = gpa_scaled;
        env.storage().persistent().set(&DataKey::Student(student_id), &student);
    }

    pub fn set_active(env: Env, caller: Address, student_id: u64, active: bool) {
        caller.require_auth();
        Self::assert_admin(&env, &caller);
        let mut student: Student = env.storage().persistent()
            .get(&DataKey::Student(student_id)).expect("student not found");
        student.is_active = active;
        env.storage().persistent().set(&DataKey::Student(student_id), &student);
    }

    pub fn release_funds(env: Env, caller: Address, student_id: u64) -> u64 {
        caller.require_auth();
        Self::assert_admin(&env, &caller);

        let mut student: Student = env.storage().persistent()
            .get(&DataKey::Student(student_id)).expect("student not found");

        if !student.is_active {
            panic!("student is not active");
        }

        let next_semester = student.current_semester + 1;
        let max_semesters: u32 = env.storage().instance().get(&DataKey::MaxSemesters).unwrap();
        if next_semester > max_semesters || next_semester > student.total_semesters {
            panic!("semester limit exceeded");
        }

        let min_gpa: u32 = env.storage().instance().get(&DataKey::MinGpaScaled).unwrap();
        if student.current_semester > 0 && student.gpa_scaled < min_gpa {
            panic!("gpa below threshold");
        }

        let dis_key = DataKey::Disbursed(student_id, next_semester);
        if env.storage().persistent().has(&dis_key) {
            panic!("already disbursed for this semester");
        }

        let locked: u64    = env.storage().instance().get(&DataKey::PoolLocked).unwrap();
        let disbursed: u64 = env.storage().instance().get(&DataKey::PoolDisbursed).unwrap();
        let amount: u64    = env.storage().instance().get(&DataKey::PerSemesterAmt).unwrap();

        if locked < disbursed + amount {
            panic!("insufficient funds in pool");
        }

        env.storage().instance().set(&DataKey::PoolDisbursed, &(disbursed + amount));
        env.storage().persistent().set(&dis_key, &true);

        student.current_semester = next_semester;
        student.total_received   += amount;
        env.storage().persistent().set(&DataKey::Student(student_id), &student);

        amount
    }

    pub fn available_balance(env: Env) -> u64 {
        let locked: u64    = env.storage().instance().get(&DataKey::PoolLocked).unwrap();
        let disbursed: u64 = env.storage().instance().get(&DataKey::PoolDisbursed).unwrap();
        locked.saturating_sub(disbursed)
    }

    pub fn get_student(env: Env, student_id: u64) -> Student {
        env.storage().persistent()
            .get(&DataKey::Student(student_id)).expect("student not found")
    }

    pub fn is_disbursed(env: Env, student_id: u64, semester: u32) -> bool {
        env.storage().persistent().has(&DataKey::Disbursed(student_id, semester))
    }

    fn assert_admin(env: &Env, caller: &Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if *caller != admin {
            panic!("unauthorized: caller is not admin");
        }
    }
}

// ══════════════════════════════════════════════
//  TESTS
// ══════════════════════════════════════════════

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, ScholarshipContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let cid    = env.register(ScholarshipContract, ());
        let client = ScholarshipContractClient::new(&env, &cid);
        let admin  = Address::generate(&env);
        client.initialize(&admin, &1_000_000u64, &100_000u64, &250u32, &8u32);
        (env, client, admin)
    }

    // ── 1. Initial balance ───────────────────────────────
    #[test]
    fn test_initial_balance() {
        let (_env, client, _admin) = setup();
        assert_eq!(client.available_balance(), 1_000_000u64);
    }

    // ── 2. Register student ──────────────────────────────
    #[test]
    fn test_register_student() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        let s = client.get_student(&1u64);
        assert_eq!(s.id,              1u64);
        assert!(s.is_active);
        assert_eq!(s.current_semester, 0u32);
        assert_eq!(s.total_received,  0u64);
    }

    // ── 3. Duplicate registration ────────────────────────
    #[test]
    #[should_panic(expected = "student already registered")]
    fn test_duplicate_registration() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        client.register_student(&admin, &1u64, &8u32);
    }

    // ── 4. Semester 1 — no GPA gate ─────────────────────
    #[test]
    fn test_semester_1_no_gpa_gate() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        let released = client.release_funds(&admin, &1u64);
        assert_eq!(released, 100_000u64);
        let s = client.get_student(&1u64);
        assert_eq!(s.current_semester, 1u32);
        assert_eq!(s.total_received,   100_000u64);
        assert_eq!(client.available_balance(), 900_000u64);
    }

    // ── 5. Semester 2 — good GPA ─────────────────────────
    #[test]
    fn test_semester_2_good_gpa() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        client.release_funds(&admin, &1u64);
        client.update_gpa(&admin, &1u64, &350u32);
        let released = client.release_funds(&admin, &1u64);
        assert_eq!(released, 100_000u64);
        assert_eq!(client.get_student(&1u64).current_semester, 2u32);
    }

    // ── 6. Low GPA withheld ──────────────────────────────
    #[test]
    #[should_panic(expected = "gpa below threshold")]
    fn test_low_gpa_withheld() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        client.release_funds(&admin, &1u64);
        client.update_gpa(&admin, &1u64, &180u32);
        client.release_funds(&admin, &1u64);
    }

    // ── 7. Duplicate disbursement guard ──────────────────
    #[test]
    fn test_disbursed_flag_set() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        assert!(!client.is_disbursed(&1u64, &1u32));
        client.release_funds(&admin, &1u64);
        assert!(client.is_disbursed(&1u64, &1u32));
        assert!(!client.is_disbursed(&1u64, &2u32));
    }

    // ── 8. Semester limit exceeded ───────────────────────
    #[test]
    #[should_panic(expected = "semester limit exceeded")]
    fn test_semester_limit() {
        let env = Env::default();
        env.mock_all_auths();
        let cid    = env.register(ScholarshipContract, ());
        let client = ScholarshipContractClient::new(&env, &cid);
        let admin  = Address::generate(&env);
        client.initialize(&admin, &1_000_000u64, &100_000u64, &200u32, &2u32);
        client.register_student(&admin, &10u64, &2u32);
        client.release_funds(&admin, &10u64);
        client.update_gpa(&admin, &10u64, &300u32);
        client.release_funds(&admin, &10u64);
        client.update_gpa(&admin, &10u64, &300u32);
        client.release_funds(&admin, &10u64); // sem 3 → panic
    }

    // ── 9. Insufficient funds ────────────────────────────
    #[test]
    #[should_panic(expected = "insufficient funds in pool")]
    fn test_insufficient_funds() {
        let env = Env::default();
        env.mock_all_auths();
        let cid    = env.register(ScholarshipContract, ());
        let client = ScholarshipContractClient::new(&env, &cid);
        let admin  = Address::generate(&env);
        client.initialize(&admin, &50_000u64, &100_000u64, &0u32, &8u32);
        client.register_student(&admin, &20u64, &8u32);
        client.release_funds(&admin, &20u64);
    }

    // ── 10. Suspended student blocked ────────────────────
    #[test]
    #[should_panic(expected = "student is not active")]
    fn test_suspended_blocked() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        client.set_active(&admin, &1u64, &false);
        client.release_funds(&admin, &1u64);
    }

    // ── 11. Invalid GPA rejected ─────────────────────────
    #[test]
    #[should_panic(expected = "gpa must be <= 4.00 (400 scaled)")]
    fn test_invalid_gpa() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        client.update_gpa(&admin, &1u64, &450u32);
    }

    // ── 12. Multiple students isolated ───────────────────
    #[test]
    fn test_multiple_students_isolated() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        client.register_student(&admin, &2u64, &8u32);
        client.release_funds(&admin, &1u64);
        client.update_gpa(&admin, &1u64, &300u32);
        client.release_funds(&admin, &1u64);
        client.release_funds(&admin, &2u64);
        assert_eq!(client.get_student(&1u64).total_received, 200_000u64);
        assert_eq!(client.get_student(&2u64).total_received, 100_000u64);
        assert_eq!(client.available_balance(), 700_000u64);
    }

    // ── 13. Reactivate suspended student ─────────────────
    #[test]
    fn test_reactivate_student() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        client.set_active(&admin, &1u64, &false);
        client.set_active(&admin, &1u64, &true);
        let released = client.release_funds(&admin, &1u64);
        assert_eq!(released, 100_000u64);
    }

    // ── 14. Unauthorized caller blocked ──────────────────
    #[test]
    #[should_panic]
    fn test_unauthorized_caller() {
        let (env, client, admin) = setup();
        client.register_student(&admin, &1u64, &8u32);
        let hacker = Address::generate(&env);
        env.mock_all_auths_allowing_non_root_auth();
        client.release_funds(&hacker, &1u64);
    }

    // ── 15. Full 8-semester journey ──────────────────────
    #[test]
    fn test_full_scholarship_journey() {
        let (_env, client, admin) = setup();
        client.register_student(&admin, &99u64, &8u32);
        for sem in 1u32..=8 {
            if sem > 1 {
                client.update_gpa(&admin, &99u64, &(300u32 + sem * 5));
            }
            let released = client.release_funds(&admin, &99u64);
            assert_eq!(released, 100_000u64);
        }
        assert_eq!(client.get_student(&99u64).total_received, 800_000u64);
        assert_eq!(client.get_student(&99u64).current_semester, 8u32);
    }
}