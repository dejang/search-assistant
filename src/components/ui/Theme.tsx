import * as React from 'react';

export type Theme = 'light' | 'dark' | 'system';

const STORAGE_KEY = 'pane-theme';
const THEMES = ['light', 'dark', 'system'] as const;

interface ThemeContextValue {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

const ThemeContext = React.createContext<ThemeContextValue | null>(null);

function readStoredTheme(): Theme {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    return (THEMES as readonly string[]).includes(stored ?? '') ? (stored as Theme) : 'system';
  } catch {
    return 'system';
  }
}

// Spec §2.3: data-theme goes on <html> only, so portal content picks it up.
function applyThemeToDocument(theme: Theme): void {
  const root = document.documentElement;
  if (theme === 'system') {
    root.removeAttribute('data-theme');
  } else {
    root.setAttribute('data-theme', theme);
  }
}

/** Pane theme provider (spec §2.3): applies data-theme to <html> and persists the choice. */
export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setTheme] = React.useState<Theme>(readStoredTheme);

  React.useEffect(() => {
    applyThemeToDocument(theme);
    try {
      localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      // Storage unavailable (e.g. private browsing): theme applies for this session only.
    }
  }, [theme]);

  const value = React.useMemo(() => ({ theme, setTheme }), [theme]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

/** Current theme and setter. Must be used within a ThemeProvider. */
export function useTheme(): ThemeContextValue {
  const context = React.useContext(ThemeContext);
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider');
  }
  return context;
}
