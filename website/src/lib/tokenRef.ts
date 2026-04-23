let token: string | null = null

export const tokenRef = {
  get: () => token,
  set: (value: string | null) => {
    token = value
  },
}
