const FRAMES = ["=^..^=", "=^-.^=", "=^..^=", "=^.-^="];
const COLOR = "\x1b[38;5;208m"; // orange
const RESET = "\x1b[0m";

export class CatSpinner {
  private frameIndex = 0;
  private timer: NodeJS.Timeout | null = null;
  private text = "";
  private readonly isTTY: boolean;

  constructor() {
    // Piped/CI output (isTTY === false) can't usefully overwrite a line —
    // fall back to plain sequential logging instead of spamming \r codes.
    this.isTTY = Boolean(process.stdout.isTTY);
  }

  start(text: string): void {
    this.text = text;
    if (!this.isTTY) {
      console.log(text);
      return;
    }
    process.stdout.write("\x1b[?25l"); // hide cursor
    this.render();
    this.timer = setInterval(() => {
      this.frameIndex = (this.frameIndex + 1) % FRAMES.length;
      this.render();
    }, 220);
  }

  update(text: string): void {
    this.text = text;
    if (!this.isTTY) {
      console.log(text);
    }
  }

  private render(): void {
    const frame = FRAMES[this.frameIndex];
    process.stdout.write(`\r${COLOR}${frame}${RESET} ${this.text}${" ".repeat(12)}`);
  }

  private stopTimer(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  succeed(text: string): void {
    this.stopTimer();
    if (this.isTTY) {
      process.stdout.write(`\r${COLOR}=^..^=${RESET} ${text}${" ".repeat(12)}\n`);
      process.stdout.write("\x1b[?25h"); // show cursor
    } else {
      console.log(text);
    }
  }

  fail(text: string): void {
    this.stopTimer();
    if (this.isTTY) {
      process.stdout.write(`\r${COLOR}=x..x=${RESET} ${text}${" ".repeat(12)}\n`);
      process.stdout.write("\x1b[?25h");
    } else {
      console.error(text);
    }
  }
}