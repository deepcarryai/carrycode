import React, { createContext, useContext, useState, useEffect } from 'react';
import { Theme } from './types.js';
import { themes } from './themes.js';
import { useRustBridge } from '../hooks/useRustBridge.js';

interface ThemeContextType {
  theme: Theme;
  setThemeName: (name: string) => void;
  availableThemes: Theme[];
}

const ThemeContext = createContext<ThemeContextType>({
  theme: themes.carrycodeDark,
  setThemeName: () => {},
  availableThemes: [],
});

export const useTheme = () => useContext(ThemeContext);

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [themeName, setThemeNameState] = useState<string>('carrycode-dark');
  const { getAppConfig, setTheme: saveTheme } = useRustBridge();

  useEffect(() => {
    try {
      const config = getAppConfig();
      const configuredTheme = config.theme || config.welcome?.theme || 'carrycode-dark';
      setThemeNameState(configuredTheme);
    } catch (e) {
      // ignore
    }
  }, []);

  const setThemeName = (name: string) => {
      setThemeNameState(name);
      saveTheme(name).catch(() => {});
  };

  const theme = Object.values(themes).find(t => t.name === themeName) || themes.carrycodeDark;

  const availableThemes = Object.values(themes);

  return (
    <ThemeContext.Provider value={{ theme, setThemeName, availableThemes }}>
      {children}
    </ThemeContext.Provider>
  );
};
