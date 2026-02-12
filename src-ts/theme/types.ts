export interface ThemeColors {
  primary: string;
  secondary: string;
  success: string;
  error: string;
  warning: string;
  info: string;
  text: string;
  dimText: string;
  border: string;
  highlight: string;
  background: string;
  diffAddFg: string;
  diffAddBg: string;
  diffRemFg: string;
  diffRemBg: string;
  bannerGradient: string | string[];
}

export interface Theme {
  name: string;
  colors: ThemeColors;
}
