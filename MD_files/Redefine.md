



### Part 1: The Transition Modules

We will break the transition into 4 modules. Do not touch the Rust core logic unless absolutely necessary.

#### Module 1: The "Skin" (UX & Terminology)
*Goal: Make the app look like a disaster tool, not a file transfer utility.*

1.  **Rename Roles (Frontend Routing):**
    *   *Old:* `Sender Mode` / `Receiver Mode`
    *   *New:* **Field Agent** (The person on the ground) / **Command Center** (The coordinators).
2.  **Visual Theme:**
    *   Switch to a "High Contrast / Emergency" theme. Dark mode with bright **Amber/Orange** accents (warning colors) or **Green** (success/resilience).
    *   Remove "ChunkStream Pro" logo. Replace with **"RESILIENT: Disaster Data Link"**.
3.  **Button Labels:**
    *   *Old:* "Upload File"
    *   *New:* "Transmit Critical Data"
    *   *Old:* "Download"
    *   *New:* "Intelligence Received"

#### Module 2: The Simulator (The "Proof")
*Goal: Your existing simulator is the most powerful part. We just need to name the scenarios correctly.*

1.  **Create "Disaster Presets" in the Network Simulator:**
    Update your simulator dropdown to use these specific presets instead of generic "F1/Telelemetry":
    *   **Scenario A: "Monsoon Flooding"** (30% Packet Loss, 800ms Latency, Jitter: High).
    *   **Scenario B: "Post-Cyclone"** (50% Packet Loss, Intermittent Connection).
    *   **Scenario C: "Congested Shelter"** (15% Packet Loss, Low Bandwidth).
2.  **Visual Feedback:**
    *   When packet loss occurs, show a visual indicator like: **"Signal Lost... Retrying via Erasure Coding"** instead of just a red graph.

#### Module 3: The Payload (What we are sending)
*Goal: Stop sending `video.mp4` or `data.zip`. Send data that tells a story.*

1.  **Create Dummy Data Files:**
    Create three specific files to include in your repo for the demo:
    *   `Sector4_Victims.csv`: A list of names, ages, and medical conditions (Hypothermia, Trauma).
    *   `FloodZone_Map.kmz`: A (fake) map file marking trapped families.
    *   `Drone_Footage_Sector2.mp4`: A short, low-res clip.
2.  **UI Metadata:**
    *   When uploading, show a "Priority Level" badge.
    *   **Critical:** "Life-Safety Data" (Red).
    *   **High:** "Damage Assessment" (Orange).
    *   **Normal:** "Logistics/Supply" (Blue).

#### Module 4: The Command Dashboard (The "Receiver")
*Goal: Make the receiver side look like a NASA mission control.*

1.  **Data Visualization:**
    *   Don't just show a list of files. Show a "Live Status Map" or a "Mission Log".
    *   *Log Entry:* `[10:02 AM] Connection established with Agent Alpha (Signal: Weak)`
    *   *Log Entry:* `[10:03 AM] Packet loss detected (40%). Switching to Recovery Mode.`
    *   *Log Entry:* `[10:05 AM] DATA RECOVERED. "Sector4_Victims.csv" ready.`
2.  **Integrity Check:**
    *   Prominently display the **BLAKE3 Hash Verification** as a "Data Integrity Certified" stamp. Judges love hearing that the data wasn't corrupted.

---

### Part 2: The Final Walkthrough (Product Demo)

This is the exact flow you (or the judge) will go through during the demo.

#### Step 1: The Setup (Establishing the Problem)
**Action:** Open the "Field Agent" (Sender) interface.
**Context:** "I am a volunteer in a flood zone. The cell towers are overloaded. I have a list of 20 people trapped on a roof."

#### Step 2: The "Baseline" Failure (The Contrast)
**Action:** Select the **"Monsoon Flooding"** network scenario in your simulator.
**Task:** Try to upload the file using a "Standard HTTP" simulation (or just show a comparison graph).
**Visual:** The progress bar hangs, spins, and fails. *"Connection Timed Out".*
**Narrative:** "Normal apps fail here. The data is lost."

#### Step 3: The Resilient Solution (The Magic)
**Action:** Click **"Transmit Critical Data"** using Resilient.
**Visual:**
1.  The file chunks into small pieces.
2.  As the simulator drops packets (turns red), you see the "Parity Shards" (the redundant data) filling in the gaps.
3.  The progress bar stutters but **keeps moving forward**.
4.  Status: *"Reconstructing file from partial data..."*

#### Step 4: The Verification (The Trust)
**Action:** Switch to the **"Command Center"** (Receiver) interface.
**Visual:**
1.  A notification pops up: **"New Intel Received: Sector4_Victims.csv"**.
2.  Click to open the CSV. It shows the list of names perfectly intact.
3.  A green badge appears: **"INTEGRITY VERIFIED (BLAKE3)"**.

#### Step 5: The Impact (The Closing)
**Narrative:** "Because we used Resilient, the Command Center now knows exactly who is trapped and can send the rescue boat. Standard technology failed; Resilient succeeded."

---

### Summary of Changes Checklist

- [ ] **Repo Rename:** `ChunkStream Pro` -> `Resilient-Disaster-Response`
- [ ] **Frontend:** Change "Sender/Receiver" to "Field Agent/Command Center".
- [ ] **Simulator:** Add "Monsoon", "Cyclone", "Earthquake" presets.
- [ ] **Assets:** Create a fake `victims.csv` file.
- [ ] **Readme:** Rewrite the "Problem Statement" to focus on the disaster volunteer story.

**This plan leverages your hard work on the Rust backend but wraps it in a package that is emotionally resonant and perfectly aligned with the hackathon criteria.**