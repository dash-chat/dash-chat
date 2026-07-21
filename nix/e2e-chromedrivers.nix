{ ... }:

{
  perSystem =
    { inputs', pkgs, ... }:
    {
      # One chromedriver per e2e Android device WebView major (physical
      # phones on 149/150, the emulator image on 124), each from its own
      # pinned nixpkgs rev. The e2e harness materializes this via
      # `nix build --out-link` and points Appium at it.
      packages.e2e-chromedrivers = pkgs.linkFarm "e2e-chromedrivers" [
        {
          name = "chromedriver-150";
          path = "${inputs'.nixpkgs-chromedriver-150.legacyPackages.chromedriver}/bin/chromedriver";
        }
        {
          name = "chromedriver-149";
          path = "${inputs'.nixpkgs-chromedriver-149.legacyPackages.chromedriver}/bin/chromedriver";
        }
        {
          name = "chromedriver-124";
          path = "${inputs'.nixpkgs-chromedriver-124.legacyPackages.chromedriver}/bin/chromedriver";
        }
      ];
    };
}
