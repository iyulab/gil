import type { Metadata } from "next";
import { Inter } from "next/font/google";
import "./globals.css";
import Link from "next/link";

const inter = Inter({ subsets: ["latin"] });

export const metadata: Metadata = {
  title: "Gil-Flow",
  description: "A powerful workflow automation engine.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={inter.className}>
        <header className="bg-gray-800 text-white p-4 shadow-md">
          <nav className="container mx-auto flex justify-between items-center">
            <Link href="/" className="text-2xl font-bold">
              Gil-Flow
            </Link>
            <ul className="flex space-x-4">
              <li>
                <Link href="/docs" className="hover:text-blue-400 transition-colors">
                  Docs
                </Link>
                <Link href="https://github.com/iyulab/gil" className="hover:text-blue-400 transition-colors ml-4">
                  GitHub
                </Link>
              </li>
            </ul>
          </nav>
        </header>
        {children}
        <footer className="bg-gray-800 text-white p-4 text-center text-sm mt-auto">
          © {new Date().getFullYear()} iyulab. All rights reserved.
        </footer>
      </body>
    </html>
  );
}