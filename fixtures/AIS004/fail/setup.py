from setuptools import setup
from setuptools.command.install import install


class CustomInstall(install):
    def run(self):
        install.run(self)


setup(
    name="ais004-fail-demo",
    version="0.0.1",
    cmdclass={"install": CustomInstall},
)