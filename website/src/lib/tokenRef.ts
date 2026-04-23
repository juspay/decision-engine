// Shared mutable ref to break circular dependency between api.ts and authStore.ts
let _token: string | null = null

export const tokenRef = {
  get: () => _token,
  set: (t: string | null) => {
    _token = t
  },
}
