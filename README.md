# d-buddy
d-buddy is a fast tui for dbus monitor using rust.
It allows you to inspect, filter and group real-time dbus signals for easy viewing/debugging.

if you have ever used dbus-monitor, you will know how tedious it is to look through and view the dbus signals coming in. 
d-buddy is a tool that makes this easier.

![img](./docs/img/d-buddy.png)

you can view system, session buses, or both:

![img](./docs/img/buses.png)


ive opted into completley removing the "signal source id" in the tui and instead use the application where the signal originated from. This will make it easier to understand where the signal came from and how it was generated.. details even has the arguments of how the application started.

## details
every signal have some details.  
ive tried to make it as easy as possible to understand what is going on (ie calling a array of dict objects, a struct and showing data as such).  

However you migh disagree, here as its "not the same type", i want to have a way to differentiate between them and have a "simplistic" view for easy debugging.
This view will have more focus going forwards.
![img](./docs/img/details.png)


## grouping
you can group, by a single or multiple types at once:

![img](./docs/img/group.png)

## filtering
filter by type of signal, sender app, path or even if the signal is a reply to another signal. 

![img](./docs/img/filter.png)

dont worry tho. you can select a active signal and grab properties to filter, so you dont have to remember any syntax.

![img](./docs/img/autofilter.png)

## app arguments
you can just start `d-buddy` for normal operation w/o any arguments.

* **log**: `--log` . also remember to set log level in env: `RUST_LOG=d_buddy=trace`
  * `RUST_LOG=d_buddy=trace `
* **enable debug ui**: `--debug-ui`
* **start and make sure it runs**: `--check`


## potential issues

### clipboard
from the details view you can copy the clipboard content to the clipboard. 
however there might be either a issue or a delay if you are on wayland. 
the package that handles this have come code paths to automatically fix this in runtime

### alot of messages (40k++)
ive tried to massage the way messages are handled and rendered in the tui, but if you have a lot of messages (40k+) the tui gets sluggish.




## Build from source
```bash
git clone https://github.com/WithHolm/d-buddy
cd d-buddy
make build
```
