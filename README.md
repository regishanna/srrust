# srrust
SkyReacher, aircraft position server.

This server provides the position of aircraft that are close to client applications like [SkyReacher for Android](https://github.com/regishanna/srandroid).

Client applications connect to the server and send their approximate location. This allows the server to only send the position of aircraft close to the client in order to optimize bandwidth. Aircraft positions are sent to clients using the GDL90 protocol.

The server retrieves the position of aircraft using the following networks:
* [OGN](https://www.glidernet.org/) for glider positions (mainly via the FLARM protocol) and for aircraft positions using the SafeSky application
* [ADSBHub](https://www.adsbhub.org/) for aircraft equipped with ADS-B transponders
