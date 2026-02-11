declare module 'ink-stepper' {
  import type React from 'react';

  export interface StepContext {
    goNext: () => void;
    goBack: () => void;
    goTo: (step: number) => void;
    cancel: () => void;
    currentStep: number;
    totalSteps: number;
    isFirst: boolean;
    isLast: boolean;
    isValidating: boolean;
  }

  export interface StepperMarkers {
    completed?: string;
    current?: string;
    pending?: string;
  }

  export interface ProgressContext {
    currentStep: number;
    steps: Array<{
      name: string;
      completed: boolean;
      current: boolean;
    }>;
  }

  export interface StepperProps {
    children: React.ReactNode;
    onComplete: () => void;
    onCancel?: () => void;
    onStepChange?: (step: number) => void;
    onEnterStep?: (step: number) => void;
    onExitStep?: (step: number) => void | boolean | Promise<boolean>;
    step?: number;
    keyboardNav?: boolean;
    showProgress?: boolean;
    renderProgress?: (ctx: ProgressContext) => React.ReactNode;
    markers?: StepperMarkers;
  }

  export function Stepper(props: StepperProps): React.ReactElement;

  export interface StepProps {
    name: string;
    canProceed?: boolean | (() => boolean | Promise<boolean>);
    children: React.ReactNode | ((ctx: StepContext) => React.ReactNode);
  }

  export function Step(props: StepProps): React.ReactElement;
}

