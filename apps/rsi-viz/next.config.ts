import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  // The app is a local, read-only viewer; it never needs image optimisation.
  images: { unoptimized: true },
};

export default nextConfig;
