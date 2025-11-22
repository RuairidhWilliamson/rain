document.querySelectorAll("time.timestamp").forEach((el) => {
  const datetime = new Date(el.getAttribute("datetime"));
  el.textContent = datetime.toLocaleString(undefined, {
    dateStyle: "short",
    timeStyle: "short",
  });
});
document.querySelectorAll("time.duration").forEach((el) => {
  const duration = Temporal.Duration.from(el.getAttribute("datetime"));
  el.textContent = duration.toLocaleString(undefined, { style: "narrow" });
});
