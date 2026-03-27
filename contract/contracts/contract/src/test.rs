// Student Scholarship Disbursement Contract
// Conditional fund release: locked funds released per semester/performance

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ScholarshipStatus {
    Locked,
    PendingReview,
    Disbursed,
    Revoked,
}

#[derive(Debug, Clone)]
pub struct SemesterRecord {
    pub semester_id: u32,
    pub gpa: f64,
    pub credits_completed: u32,
    pub attendance_pct: f64,
    pub disbursed: bool,
    pub amount: u64, // in smallest unit (e.g., paise / cents)
}

#[derive(Debug, Clone)]
pub struct Student {
    pub id: String,
    pub name: String,
    pub total_locked_funds: u64,
    pub released_funds: u64,
    pub status: ScholarshipStatus,
    pub semesters: Vec<SemesterRecord>,
}

#[derive(Debug)]
pub struct ScholarshipContract {
    pub admin: String,
    pub students: HashMap<String, Student>,
    pub min_gpa: f64,
    pub min_attendance: f64,
    pub min_credits: u32,
    pub per_semester_amount: u64,
}

impl ScholarshipContract {
    pub fn new(
        admin: String,
        min_gpa: f64,
        min_attendance: f64,
        min_credits: u32,
        per_semester_amount: u64,
    ) -> Self {
        ScholarshipContract {
            admin,
            students: HashMap::new(),
            min_gpa,
            min_attendance,
            min_credits,
            per_semester_amount,
        }
    }

    /// Register a student and lock their total scholarship funds
    pub fn register_student(
        &mut self,
        caller: &str,
        student_id: String,
        name: String,
        total_semesters: u32,
    ) -> Result<String, String> {
        if caller != self.admin {
            return Err("Only admin can register students".to_string());
        }
        if self.students.contains_key(&student_id) {
            return Err(format!("Student {} already registered", student_id));
        }
        let total_locked = self.per_semester_amount * total_semesters as u64;
        let student = Student {
            id: student_id.clone(),
            name: name.clone(),
            total_locked_funds: total_locked,
            released_funds: 0,
            status: ScholarshipStatus::Locked,
            semesters: vec![],
        };
        self.students.insert(student_id.clone(), student);
        Ok(format!(
            "Student '{}' registered. Total locked funds: {} units across {} semesters.",
            name, total_locked, total_semesters
        ))
    }

    /// Submit semester performance — triggers conditional release check
    pub fn submit_semester_performance(
        &mut self,
        caller: &str,
        student_id: &str,
        semester_id: u32,
        gpa: f64,
        credits_completed: u32,
        attendance_pct: f64,
    ) -> Result<String, String> {
        if caller != self.admin {
            return Err("Only admin can submit performance".to_string());
        }
        let student = self
            .students
            .get_mut(student_id)
            .ok_or_else(|| format!("Student {} not found", student_id))?;

        // Check duplicate semester
        if student.semesters.iter().any(|s| s.semester_id == semester_id) {
            return Err(format!("Semester {} already recorded", semester_id));
        }

        let passed = gpa >= self.min_gpa
            && attendance_pct >= self.min_attendance
            && credits_completed >= self.min_credits;

        let amount = if passed { self.per_semester_amount } else { 0 };

        if passed {
            student.released_funds += amount;
            student.status = ScholarshipStatus::Disbursed;
        } else {
            student.status = ScholarshipStatus::PendingReview;
        }

        student.semesters.push(SemesterRecord {
            semester_id,
            gpa,
            credits_completed,
            attendance_pct,
            disbursed: passed,
            amount,
        });

        if passed {
            Ok(format!(
                "✅ Semester {} passed. {} units disbursed to student '{}'.",
                semester_id, amount, student.name
            ))
        } else {
            Ok(format!(
                "❌ Semester {} failed criteria. Funds remain locked for student '{}'.\n  GPA: {:.2} (min {:.2}), Attendance: {:.1}% (min {:.1}%), Credits: {} (min {})",
                semester_id, student.name, gpa, self.min_gpa,
                attendance_pct, self.min_attendance,
                credits_completed, self.min_credits
            ))
        }
    }

    /// Query student scholarship summary
    pub fn get_student_summary(&self, student_id: &str) -> Result<String, String> {
        let student = self
            .students
            .get(student_id)
            .ok_or_else(|| format!("Student {} not found", student_id))?;

        let locked = student.total_locked_funds - student.released_funds;
        let mut summary = format!(
            "=== Scholarship Summary: {} ({}) ===\nStatus: {:?}\nTotal Locked: {} | Released: {} | Remaining: {}\n\nSemester Records:\n",
            student.name, student.id, student.status,
            student.total_locked_funds, student.released_funds, locked
        );
        for s in &student.semesters {
            summary.push_str(&format!(
                "  Sem {}: GPA={:.2}, Credits={}, Attendance={:.1}% → {} ({})\n",
                s.semester_id,
                s.gpa,
                s.credits_completed,
                s.attendance_pct,
                if s.disbursed { "DISBURSED" } else { "LOCKED" },
                if s.disbursed {
                    format!("{} units", s.amount)
                } else {
                    "0 units".to_string()
                }
            ));
        }
        Ok(summary)
    }

    /// Revoke scholarship (admin only)
    pub fn revoke_scholarship(&mut self, caller: &str, student_id: &str) -> Result<String, String> {
        if caller != self.admin {
            return Err("Only admin can revoke scholarships".to_string());
        }
        let student = self
            .students
            .get_mut(student_id)
            .ok_or_else(|| format!("Student {} not found", student_id))?;
        student.status = ScholarshipStatus::Revoked;
        Ok(format!("Scholarship revoked for student '{}'.", student.name))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_contract() -> ScholarshipContract {
        ScholarshipContract::new(
            "admin".to_string(),
            7.0,   // min GPA
            75.0,  // min attendance %
            18,    // min credits
            50000, // per semester amount
        )
    }

    #[test]
    fn test_register_student() {
        let mut contract = setup_contract();
        let result = contract.register_student("admin", "S001".to_string(), "Alice".to_string(), 8);
        assert!(result.is_ok());
        assert!(contract.students.contains_key("S001"));
        let student = &contract.students["S001"];
        assert_eq!(student.total_locked_funds, 400_000); // 50000 * 8
        assert_eq!(student.status, ScholarshipStatus::Locked);
    }

    #[test]
    fn test_only_admin_can_register() {
        let mut contract = setup_contract();
        let result =
            contract.register_student("hacker", "S002".to_string(), "Bob".to_string(), 4);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Only admin can register students");
    }

    #[test]
    fn test_disburse_on_pass() {
        let mut contract = setup_contract();
        contract
            .register_student("admin", "S001".to_string(), "Alice".to_string(), 8)
            .unwrap();
        let result =
            contract.submit_semester_performance("admin", "S001", 1, 8.5, 21, 85.0);
        assert!(result.is_ok());
        let student = &contract.students["S001"];
        assert_eq!(student.released_funds, 50_000);
        assert_eq!(student.status, ScholarshipStatus::Disbursed);
        assert!(student.semesters[0].disbursed);
    }

    #[test]
    fn test_lock_on_fail_gpa() {
        let mut contract = setup_contract();
        contract
            .register_student("admin", "S001".to_string(), "Alice".to_string(), 8)
            .unwrap();
        let result =
            contract.submit_semester_performance("admin", "S001", 1, 5.5, 20, 80.0);
        assert!(result.is_ok());
        let student = &contract.students["S001"];
        assert_eq!(student.released_funds, 0);
        assert_eq!(student.status, ScholarshipStatus::PendingReview);
        assert!(!student.semesters[0].disbursed);
    }

    #[test]
    fn test_lock_on_low_attendance() {
        let mut contract = setup_contract();
        contract
            .register_student("admin", "S001".to_string(), "Alice".to_string(), 8)
            .unwrap();
        let result =
            contract.submit_semester_performance("admin", "S001", 1, 8.0, 20, 60.0);
        assert!(result.is_ok());
        let student = &contract.students["S001"];
        assert_eq!(student.released_funds, 0);
    }

    #[test]
    fn test_multi_semester_partial_release() {
        let mut contract = setup_contract();
        contract
            .register_student("admin", "S001".to_string(), "Alice".to_string(), 4)
            .unwrap();
        contract
            .submit_semester_performance("admin", "S001", 1, 8.0, 20, 80.0)
            .unwrap(); // pass
        contract
            .submit_semester_performance("admin", "S001", 2, 5.0, 18, 75.0)
            .unwrap(); // fail (gpa)
        contract
            .submit_semester_performance("admin", "S001", 3, 7.5, 19, 78.0)
            .unwrap(); // pass
        let student = &contract.students["S001"];
        assert_eq!(student.released_funds, 100_000); // 2 semesters passed
        assert_eq!(student.semesters.len(), 3);
    }

    #[test]
    fn test_duplicate_semester_rejected() {
        let mut contract = setup_contract();
        contract
            .register_student("admin", "S001".to_string(), "Alice".to_string(), 4)
            .unwrap();
        contract
            .submit_semester_performance("admin", "S001", 1, 8.0, 20, 80.0)
            .unwrap();
        let result = contract.submit_semester_performance("admin", "S001", 1, 9.0, 22, 90.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already recorded"));
    }

    #[test]
    fn test_revoke_scholarship() {
        let mut contract = setup_contract();
        contract
            .register_student("admin", "S001".to_string(), "Alice".to_string(), 4)
            .unwrap();
        let result = contract.revoke_scholarship("admin", "S001");
        assert!(result.is_ok());
        assert_eq!(contract.students["S001"].status, ScholarshipStatus::Revoked);
    }

    #[test]
    fn test_student_summary() {
        let mut contract = setup_contract();
        contract
            .register_student("admin", "S001".to_string(), "Alice".to_string(), 4)
            .unwrap();
        contract
            .submit_semester_performance("admin", "S001", 1, 8.0, 20, 82.0)
            .unwrap();
        let summary = contract.get_student_summary("S001");
        assert!(summary.is_ok());
        let s = summary.unwrap();
        assert!(s.contains("Alice"));
        assert!(s.contains("DISBURSED"));
    }

    #[test]
    fn test_unknown_student_error() {
        let contract = setup_contract();
        let result = contract.get_student_summary("UNKNOWN");
        assert!(result.is_err());
    }
}
