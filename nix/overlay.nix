{ self, ... }:
{
  overlays = {
    default = final: _prev: {
      moq = self.packages.${final.system};
    };
  };
}
