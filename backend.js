// ─── Student Scholarship Disbursement — Backend API ─────────────────────────
// Mirrors the Rust contract logic in Node.js for the frontend to consume.

const express = require("express");
const cors = require("cors");
const app = express();

app.use(cors());
app.use(express.json());

// ─── In-Memory State (mirrors Rust contract) ──────────────────────────────────

const CONTRACT = {
  admin: "admin",
  minGpa: 7.0,
  minAttendance: 75.0,
  minCredits: 18,
  perSemesterAmount: 50000, // units
  students: {},
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

function requireAdmin(caller, res) {
  if (caller !== CONTRACT.admin) {
    res.status(403).json({ error: "Only admin can perform this action" });
    return false;
  }
  return true;
}

// ─── Routes ───────────────────────────────────────────────────────────────────

// GET /api/config — contract parameters
app.get("/api/config", (req, res) => {
  res.json({
    minGpa: CONTRACT.minGpa,
    minAttendance: CONTRACT.minAttendance,
    minCredits: CONTRACT.minCredits,
    perSemesterAmount: CONTRACT.perSemesterAmount,
  });
});

// GET /api/students — list all students
app.get("/api/students", (req, res) => {
  res.json(Object.values(CONTRACT.students));
});

// GET /api/students/:id — single student summary
app.get("/api/students/:id", (req, res) => {
  const student = CONTRACT.students[req.params.id];
  if (!student) return res.status(404).json({ error: "Student not found" });
  res.json(student);
});

// POST /api/students/register — register student (admin only)
app.post("/api/students/register", (req, res) => {
  const { caller, studentId, name, totalSemesters } = req.body;
  if (!requireAdmin(caller, res)) return;
  if (CONTRACT.students[studentId])
    return res.status(409).json({ error: `Student ${studentId} already registered` });

  const totalLocked = CONTRACT.perSemesterAmount * totalSemesters;
  CONTRACT.students[studentId] = {
    id: studentId,
    name,
    totalLockedFunds: totalLocked,
    releasedFunds: 0,
    status: "Locked",
    semesters: [],
    registeredAt: new Date().toISOString(),
  };

  res.status(201).json({
    message: `Student '${name}' registered. Total locked: ${totalLocked} units across ${totalSemesters} semesters.`,
    student: CONTRACT.students[studentId],
  });
});

// POST /api/students/:id/semester — submit semester performance
app.post("/api/students/:id/semester", (req, res) => {
  const { caller, semesterId, gpa, creditsCompleted, attendancePct } = req.body;
  if (!requireAdmin(caller, res)) return;

  const student = CONTRACT.students[req.params.id];
  if (!student) return res.status(404).json({ error: "Student not found" });

  if (student.semesters.find((s) => s.semesterId === semesterId))
    return res.status(409).json({ error: `Semester ${semesterId} already recorded` });

  const passed =
    gpa >= CONTRACT.minGpa &&
    attendancePct >= CONTRACT.minAttendance &&
    creditsCompleted >= CONTRACT.minCredits;

  const amount = passed ? CONTRACT.perSemesterAmount : 0;

  if (passed) {
    student.releasedFunds += amount;
    student.status = "Disbursed";
  } else {
    student.status = "PendingReview";
  }

  const record = {
    semesterId,
    gpa,
    creditsCompleted,
    attendancePct,
    disbursed: passed,
    amount,
    submittedAt: new Date().toISOString(),
    failReasons: [],
  };

  if (gpa < CONTRACT.minGpa) record.failReasons.push(`GPA ${gpa.toFixed(2)} < ${CONTRACT.minGpa}`);
  if (attendancePct < CONTRACT.minAttendance)
    record.failReasons.push(`Attendance ${attendancePct.toFixed(1)}% < ${CONTRACT.minAttendance}%`);
  if (creditsCompleted < CONTRACT.minCredits)
    record.failReasons.push(`Credits ${creditsCompleted} < ${CONTRACT.minCredits}`);

  student.semesters.push(record);

  res.json({
    message: passed
      ? `✅ Semester ${semesterId} passed. ${amount} units disbursed.`
      : `❌ Semester ${semesterId} failed. Funds remain locked.`,
    record,
    student,
  });
});

// POST /api/students/:id/revoke — revoke scholarship
app.post("/api/students/:id/revoke", (req, res) => {
  const { caller } = req.body;
  if (!requireAdmin(caller, res)) return;
  const student = CONTRACT.students[req.params.id];
  if (!student) return res.status(404).json({ error: "Student not found" });
  student.status = "Revoked";
  res.json({ message: `Scholarship revoked for '${student.name}'.`, student });
});

// GET /api/stats — dashboard aggregates
app.get("/api/stats", (req, res) => {
  const students = Object.values(CONTRACT.students);
  const totalLocked = students.reduce((a, s) => a + s.totalLockedFunds, 0);
  const totalReleased = students.reduce((a, s) => a + s.releasedFunds, 0);
  const allSemesters = students.flatMap((s) => s.semesters);
  const disbursedCount = allSemesters.filter((s) => s.disbursed).length;
  const lockedCount = allSemesters.filter((s) => !s.disbursed).length;

  res.json({
    totalStudents: students.length,
    totalLocked,
    totalReleased,
    remainingLocked: totalLocked - totalReleased,
    disbursedSemesters: disbursedCount,
    lockedSemesters: lockedCount,
    disbursalRate:
      allSemesters.length > 0
        ? ((disbursedCount / allSemesters.length) * 100).toFixed(1)
        : 0,
  });
});

// ─── Seed some demo data ───────────────────────────────────────────────────────
function seed() {
  const students = [
    { id: "S001", name: "Arjun Mehta", semesters: 8 },
    { id: "S002", name: "Priya Sharma", semesters: 8 },
    { id: "S003", name: "Rohit Verma", semesters: 6 },
  ];
  students.forEach(({ id, name, semesters }) => {
    CONTRACT.students[id] = {
      id,
      name,
      totalLockedFunds: CONTRACT.perSemesterAmount * semesters,
      releasedFunds: 0,
      status: "Locked",
      semesters: [],
      registeredAt: new Date().toISOString(),
    };
  });

  // Arjun: 3 semesters (pass, fail, pass)
  const arjunData = [
    { sem: 1, gpa: 8.2, credits: 21, att: 88 },
    { sem: 2, gpa: 5.8, credits: 20, att: 72 },
    { sem: 3, gpa: 7.9, credits: 19, att: 81 },
  ];
  arjunData.forEach(({ sem, gpa, credits, att }) => {
    const passed = gpa >= 7.0 && att >= 75.0 && credits >= 18;
    if (passed) CONTRACT.students["S001"].releasedFunds += CONTRACT.perSemesterAmount;
    CONTRACT.students["S001"].status = passed ? "Disbursed" : "PendingReview";
    CONTRACT.students["S001"].semesters.push({
      semesterId: sem, gpa, creditsCompleted: credits, attendancePct: att,
      disbursed: passed, amount: passed ? CONTRACT.perSemesterAmount : 0,
      submittedAt: new Date().toISOString(), failReasons: [],
    });
  });

  // Priya: 2 semesters (both pass)
  [{ sem: 1, gpa: 9.1, credits: 22, att: 95 }, { sem: 2, gpa: 8.7, credits: 21, att: 91 }].forEach(
    ({ sem, gpa, credits, att }) => {
      CONTRACT.students["S002"].releasedFunds += CONTRACT.perSemesterAmount;
      CONTRACT.students["S002"].status = "Disbursed";
      CONTRACT.students["S002"].semesters.push({
        semesterId: sem, gpa, creditsCompleted: credits, attendancePct: att,
        disbursed: true, amount: CONTRACT.perSemesterAmount,
        submittedAt: new Date().toISOString(), failReasons: [],
      });
    }
  );
}

seed();

const PORT = 3001;
app.listen(PORT, () =>
  console.log(`🎓 Scholarship API running at http://localhost:${PORT}`)
);
