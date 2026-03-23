# 🔍 Context
**Issue:** #

**Motivation:** # 🛠️ Solution
- [ ] Refactored the `AuthService` to cache tokens.
- [ ] Added a new `HelmRelease` for the payment service.
- [ ] Updated Terraform state for the RDS instance.

# 📸 Screenshots / Evidence
| Before | After |
| :--- | :--- |
| | |

# 🧪 How to Test
1. Run `make test`
2. Call the endpoint `GET /api/v1/user` with header `x-test: true`
3. Verify the logs show "Token Cached"

# ⚠️ Risk & Breaking Changes
- [ ] **No breaking changes.**
- [ ] **Breaking change:** (Describe impact: e.g., "API clients must send new header")
- [ ] **Ops Requirement:** (e.g., "Needs new secret `API_KEY` in Vault before merge")

---

### 🛡️ Safeguards
- [ ] I have updated the documentation (README / Wiki).
- [ ] I have added/updated Unit Tests.
- [ ] **Security:** I have verified no secrets are hardcoded.
- [ ] **Performance:** I have verified this doesn't introduce N+1 queries or memory leaks.
