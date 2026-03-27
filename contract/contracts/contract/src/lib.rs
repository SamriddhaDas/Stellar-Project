#![no_std]
// FIX 1: Removed unused `String` and `Symbol` imports
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, symbol_short};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Pool,
    Student(Address),
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ScholarshipPool {
    pub total_locked: i128,
    pub disbursed: i128,
    pub min_gpa_scaled: u32, // e.g., 300 for 3.00
    pub amount_per_semester: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct StudentRecord {
    pub current_semester: u32,
    pub last_gpa_scaled: u32,
    pub is_active: bool,
    pub total_received: i128,
}

#[contract]
pub struct ScholarshipContract;

#[contractimpl]
impl ScholarshipContract {
    // Initialize the contract with an admin and pool settings
    pub fn init(env: Env, admin: Address, min_gpa: u32, amt: i128) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        
        let pool = ScholarshipPool {
            total_locked: 0,
            disbursed: 0,
            min_gpa_scaled: min_gpa,
            amount_per_semester: amt,
        };
        env.storage().instance().set(&DataKey::Pool, &pool);
    }

    // Admin deposits funds into the contract
    pub fn deposit(env: Env, admin: Address, amount: i128) {
        admin.require_auth();
        // FIX 2: Added a proper expect message in case pool isn't initialized
        let mut pool: ScholarshipPool = env.storage().instance().get(&DataKey::Pool).expect("Pool not initialized");
        pool.total_locked += amount;
        env.storage().instance().set(&DataKey::Pool, &pool);
    }

    // Register a student (Admin only)
    pub fn reg_student(env: Env, admin: Address, student: Address) {
        admin.require_auth();
        let record = StudentRecord {
            current_semester: 0,
            last_gpa_scaled: 0,
            is_active: true,
            total_received: 0,
        };
        env.storage().persistent().set(&DataKey::Student(student), &record);
    }

    // THE 6-GATE DISBURSEMENT LOGIC
    pub fn release_funds(env: Env, admin: Address, student_addr: Address, current_gpa: u32) {
        admin.require_auth();

        let mut student: StudentRecord = env.storage().persistent()
            .get(&DataKey::Student(student_addr.clone()))
            .expect("Gate 1: Student not found");

        if !student.is_active { panic!("Gate 2: Student not active"); }
        if student.current_semester >= 8 { panic!("Gate 3: Degree completed"); }

        let pool: ScholarshipPool = env.storage().instance().get(&DataKey::Pool).expect("Pool not found");
        
        // Gate 4: Performance (Skip for Sem 1)
        if student.current_semester > 0 && current_gpa < pool.min_gpa_scaled {
            panic!("Gate 4: GPA below threshold");
        }

        // Gate 6: Solvency
        if pool.total_locked - pool.disbursed < pool.amount_per_semester {
            panic!("Gate 6: Insufficient pool funds");
        }

        // Logic Updates
        student.current_semester += 1;
        student.last_gpa_scaled = current_gpa;
        student.total_received += pool.amount_per_semester;
        
        let mut new_pool = pool;
        new_pool.disbursed += new_pool.amount_per_semester;

        // FIX 3: Added .clone() to student_addr so it isn't consumed before the event
        env.storage().persistent().set(&DataKey::Student(student_addr.clone()), &student);
        env.storage().instance().set(&DataKey::Pool, &new_pool);
        
        // Emit Event for transparency
        env.events().publish((symbol_short!("release"), student_addr), new_pool.amount_per_semester);
    }
}