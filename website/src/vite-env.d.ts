/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_DEFAULT_TENANT_ID?: string
  readonly VITE_API_BASE_PATH?: string
  readonly VITE_FEATURE_HEADER?: string
  readonly VITE_APP_ENV?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
