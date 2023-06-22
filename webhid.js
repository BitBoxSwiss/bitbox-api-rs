export async function jsSleep(ms) {
    await new Promise(resolve => setTimeout(resolve, ms));
}

class MessageQueue {
  constructor() {
    this.queue = [];
    this.resolvers = [];
  }

  addMessage(msg) {
    if (this.resolvers.length > 0) {
      const resolveFunc = this.resolvers.shift();
      resolveFunc(msg);
    } else {
      this.queue.push(msg);
    }
  }

  getNextMessage() {
    return new Promise((resolve) => {
      if (this.queue.length > 0) {
        const msg = this.queue.shift();
        resolve(msg);
      } else {
        this.resolvers.push(resolve);
      }
    });
  }
}

export async function getWebHIDDevice(vendorId, productId) {
  let device;
  try {
    const devices = await navigator.hid.requestDevice({filters: [{vendorId, productId}]});
    const d = devices[0];
    // Filter out other products that might be in the list presented by the Browser.
    if (d.productName.includes('BitBox02')) {
      device = d;
    }
  } catch (err) {
    return null;
  }
  if (!device) {
    return null;
  }
  await device.open();

  const msgQueue = new MessageQueue();


  const onInputReport = event => {
    msgQueue.addMessage(new Uint8Array(event.data.buffer));
  };
  device.addEventListener("inputreport", onInputReport);
  return {
    write: bytes => {
      if (!device.opened) {
        console.error("attempted write to a closed HID connection");
        return;
      }
      device.sendReport(0, bytes);
    },
    read: async () => {
      return await msgQueue.getNextMessage();
    },
    close: () => {
      device.close().then(() => {
        device.removeEventListener("inputreport", onInputReport);
      });
    },
    valid: () => device.opened,
  };
}
