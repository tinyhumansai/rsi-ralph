import type { Metadata } from "next";
import Link from "next/link";
import type { ReactNode } from "react";
import "./globals.css";

export const metadata: Metadata = {
  title: "RSI Ralph Loop Viewer",
  description: "Read-only live view of RSI Ralph loop traces and run snapshots",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        <div className="shell">
          <header className="topbar">
            <Link href="/">RSI Ralph loop viewer</Link>
          </header>
          <main>{children}</main>
        </div>
      </body>
    </html>
  );
}
